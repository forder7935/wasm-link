//! Async-capable plugin runtimes.
//!
//! These types form a public API separate from the synchronous types at the
//! crate root. A graph cannot mix instances or bindings from the two runtimes.
//!
//! The separation is enforced by distinct types:
//!
//! ```compile_fail
//! use wasm_link::{ PluginContext, ResourceTable };
//! struct Context( ResourceTable );
//! impl PluginContext for Context {
//! 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.0 }
//! }
//! fn use_sync( _instance: wasm_link::PluginInstance<Context> ) {}
//! fn cannot_mix( instance: wasm_link::concurrent::PluginInstance<Context> ) {
//! 	use_sync( instance );
//! }
//! ```
//!
//! A root function also cannot be marked async:
//!
//! ```compile_fail
//! use wasm_link::{ Function, FunctionKind, ReturnKind };
//! let _ = Function::new_async( FunctionKind::Freestanding, ReturnKind::Void );
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::task::Spawn;
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Engine, Store};

use crate::cardinality::{Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne};
use crate::interface::{Function as FunctionMetadata, Interface as InterfaceMetadata};
use crate::plugin_instance::DispatchError as CoreDispatchError;
use crate::plugin_instance::PluginInstanceAsync;
use crate::{FunctionKind, PluginContext, Remap, ReturnKind};

/// An instantiated plugin in the async-capable runtime.
pub use crate::plugin_instance::PluginInstanceAsync as PluginInstance;

/// Errors produced by dispatch in an async-capable runtime.
#[derive(thiserror::Error, Debug)]
pub enum DispatchError {
    /// The interface path is unknown.
    #[error("Invalid Interface Path: {0}")]
    InvalidInterfacePath(String),
    /// The function is unknown.
    #[error("Invalid Function: {0}")]
    InvalidFunction(String),
    /// A required return value was absent.
    #[error("Missing Response")]
    MissingResponse,
    /// WebAssembly execution failed.
    #[error("Runtime Exception")]
    RuntimeException(wasmtime::Error),
    /// Arguments did not match the function signature.
    #[error("Invalid Argument List")]
    InvalidArgumentList,
    /// The value uses an unsupported Component Model type.
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
    /// The executor rejected the destination drain task.
    #[error("Async executor unavailable")]
    ExecutorUnavailable,
    /// A caller or destination queue reached its count or byte limit.
    #[error("Dispatch queue full")]
    DispatchQueueFull,
    /// A resource handle could not be created.
    #[error("Resource Create Error: {0}")]
    ResourceCreationError(#[from] crate::ResourceCreationError),
    /// A resource handle could not be received.
    #[error("Resource Receive Error: {0}")]
    ResourceReceiveError(#[from] crate::ResourceReceiveError),
}

impl From<CoreDispatchError> for DispatchError {
    fn from(error: CoreDispatchError) -> Self {
        match error {
            CoreDispatchError::InvalidInterfacePath(value) => Self::InvalidInterfacePath(value),
            CoreDispatchError::InvalidFunction(value) => Self::InvalidFunction(value),
            CoreDispatchError::MissingResponse => Self::MissingResponse,
            CoreDispatchError::RuntimeException(value) => Self::RuntimeException(value),
            CoreDispatchError::InvalidArgumentList => Self::InvalidArgumentList,
            CoreDispatchError::UnsupportedType(value) => Self::UnsupportedType(value),
            CoreDispatchError::ExecutorUnavailable => Self::ExecutorUnavailable,
            CoreDispatchError::DispatchQueueFull => Self::DispatchQueueFull,
            CoreDispatchError::ResourceCreationError(value) => Self::ResourceCreationError(value),
            CoreDispatchError::ResourceReceiveError(value) => Self::ResourceReceiveError(value),
        }
    }
}

impl From<DispatchError> for Val {
    fn from(error: DispatchError) -> Self {
        match error {
            DispatchError::InvalidInterfacePath(value) => Self::Variant(
                "invalid-interface-path".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::InvalidFunction(value) => Self::Variant(
                "invalid-function".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::MissingResponse => Self::Variant("missing-response".to_string(), None),
            DispatchError::RuntimeException(value) => Self::Variant(
                "runtime-exception".to_string(),
                Some(Box::new(Self::String(value.to_string()))),
            ),
            DispatchError::InvalidArgumentList => {
                Self::Variant("invalid-argument-list".to_string(), None)
            }
            DispatchError::UnsupportedType(value) => Self::Variant(
                "unsupported-type".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::ExecutorUnavailable => {
                Self::Variant("executor-unavailable".to_string(), None)
            }
            DispatchError::DispatchQueueFull => {
                Self::Variant("dispatch-queue-full".to_string(), None)
            }
            DispatchError::ResourceCreationError(value) => value.into(),
            DispatchError::ResourceReceiveError(value) => value.into(),
        }
    }
}

/// Function metadata accepted by an async-capable runtime.
#[derive(Debug, Clone)]
pub enum Function {
    /// A normal synchronous WIT function.
    Sync(crate::Function),
    /// A WIT function declared with the `async` effect.
    Async(crate::Function),
}

impl Function {
    /// Creates metadata for a synchronous WIT function.
    pub fn new(kind: FunctionKind, return_kind: ReturnKind) -> Self {
        Self::Sync(crate::Function::new(kind, return_kind))
    }

    /// Creates metadata for a WIT function declared with the `async` effect.
    pub fn new_async(kind: FunctionKind, return_kind: ReturnKind) -> Self {
        Self::Async(crate::Function::new(kind, return_kind))
    }

    /// The function's return kind.
    pub fn return_kind(&self) -> ReturnKind {
        self.sync_metadata().return_kind()
    }

    /// Whether this is a freestanding function or resource method.
    pub fn kind(&self) -> FunctionKind {
        self.sync_metadata().kind()
    }

    /// Whether the WIT function has the `async` effect.
    pub fn is_async(&self) -> bool {
        matches!(self, Self::Async(_))
    }

    fn sync_metadata(&self) -> &crate::Function {
        match self {
            Self::Sync(function) | Self::Async(function) => function,
        }
    }

    pub(crate) fn into_metadata(self) -> FunctionMetadata {
        match self {
            Self::Sync(function) => function.metadata().clone(),
            Self::Async(function) => {
                FunctionMetadata::new_async(function.kind(), function.return_kind())
            }
        }
    }
}

/// A WIT interface for an async-capable runtime.
#[derive(Debug, Clone, Default)]
pub struct Interface {
    metadata: InterfaceMetadata,
}

impl Interface {
    /// Creates an interface that may contain both sync and async functions.
    pub fn new(functions: HashMap<String, Function>, resources: HashSet<String>) -> Self {
        Self {
            metadata: InterfaceMetadata::new(
                functions
                    .into_iter()
                    .map(|(name, function)| (name, function.into_metadata()))
                    .collect(),
                resources,
            ),
        }
    }

    pub(crate) fn into_metadata(self) -> InterfaceMetadata {
        self.metadata
    }
}

type PluginSockets<Id, Ctx, Plugins> =
    <Plugins as Cardinality<Id, PluginInstanceAsync<Ctx>>>::Rebind<Arc<PluginInstanceAsync<Ctx>>>;
type Results<Id, Ctx, Plugins> = <PluginSockets<Id, Ctx, Plugins> as Cardinality<
    Id,
    Arc<PluginInstanceAsync<Ctx>>,
>>::Rebind<Result<Val, DispatchError>>;
type CoreResults<Id, Ctx, Plugins> = <PluginSockets<Id, Ctx, Plugins> as Cardinality<
    Id,
    Arc<PluginInstanceAsync<Ctx>>,
>>::Rebind<Result<Val, CoreDispatchError>>;

/// A binding in the async-capable runtime.
pub struct Binding<Id, Ctx, Plugins = ExactlyOne<Id, PluginInstanceAsync<Ctx>>>(
    pub(crate) crate::binding::Binding<Id, Ctx, Plugins, PluginInstanceAsync<Ctx>>,
)
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync;

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Cardinality<Id, Arc<PluginInstanceAsync<Ctx>>> + Send + Sync,
    CoreResults<Id, Ctx, Plugins>: Cardinality<
        Id,
        Result<Val, CoreDispatchError>,
        Rebind<Result<Val, DispatchError>> = Results<Id, Ctx, Plugins>,
    >,
{
    /// Creates an async-capable binding.
    pub fn new(
        package_name: impl Into<String>,
        interfaces: HashMap<String, Interface>,
        plugins: Plugins,
    ) -> Self {
        Self(crate::binding::Binding::new(
            package_name,
            interfaces
                .into_iter()
                .map(|(name, interface)| (name, interface.into_metadata()))
                .collect(),
            plugins,
        ))
    }

    /// Dispatches a call through the destination's caller-aware queue.
    ///
    /// # Errors
    ///
    /// Returns an error when the interface or function is unknown. Per-plugin
    /// execution and queue errors are returned inside the cardinality wrapper.
    pub async fn dispatch(
        &self,
        interface: &str,
        function: &str,
        args: &[Val],
    ) -> Result<Results<Id, Ctx, Plugins>, DispatchError>
    where
        Id: Into<Val>,
        Results<Id, Ctx, Plugins>: Send,
        CoreResults<Id, Ctx, Plugins>: Send,
    {
        self.0
            .dispatch_async(interface, function, args)
            .await
            .map(|results| results.map_mut(|result| result.map_err(Into::into)))
            .map_err(Into::into)
    }
}

impl<Id, Ctx, Plugins> Clone for Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Id, Ctx, Plugins> std::fmt::Debug for Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
    Ctx: PluginContext + std::fmt::Debug + 'static,
    Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync + std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// An async-capable binding with erased cardinality.
#[derive(Debug)]
pub enum BindingAny<Id, Ctx>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    /// Exactly one implementation.
    ExactlyOne(Binding<Id, Ctx, ExactlyOne<Id, PluginInstanceAsync<Ctx>>>),
    /// Zero or one implementation.
    AtMostOne(Binding<Id, Ctx, AtMostOne<Id, PluginInstanceAsync<Ctx>>>),
    /// One or more implementations.
    AtLeastOne(Binding<Id, Ctx, AtLeastOne<Id, PluginInstanceAsync<Ctx>>>),
    /// Any number of implementations.
    Any(Binding<Id, Ctx, Any<Id, PluginInstanceAsync<Ctx>>>),
}

impl<Id, Ctx> BindingAny<Id, Ctx>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn into_core(self) -> crate::binding::BindingAny<Id, Ctx, PluginInstanceAsync<Ctx>> {
        match self {
            Self::ExactlyOne(binding) => crate::binding::BindingAny::ExactlyOne(binding.0),
            Self::AtMostOne(binding) => crate::binding::BindingAny::AtMostOne(binding.0),
            Self::AtLeastOne(binding) => crate::binding::BindingAny::AtLeastOne(binding.0),
            Self::Any(binding) => crate::binding::BindingAny::Any(binding.0),
        }
    }
}

macro_rules! binding_from {
    ( $variant:ident, $cardinality:ident ) => {
        impl<Id, Ctx> From<Binding<Id, Ctx, $cardinality<Id, PluginInstanceAsync<Ctx>>>>
            for BindingAny<Id, Ctx>
        where
            Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
            Ctx: PluginContext + 'static,
        {
            fn from(binding: Binding<Id, Ctx, $cardinality<Id, PluginInstanceAsync<Ctx>>>) -> Self {
                Self::$variant(binding)
            }
        }
    };
}
binding_from!(ExactlyOne, ExactlyOne);
binding_from!(AtMostOne, AtMostOne);
binding_from!(AtLeastOne, AtLeastOne);
binding_from!(Any, Any);

impl<Id, Ctx> Clone for BindingAny<Id, Ctx>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn clone(&self) -> Self {
        match self {
            Self::ExactlyOne(binding) => Self::ExactlyOne(binding.clone()),
            Self::AtMostOne(binding) => Self::AtMostOne(binding.clone()),
            Self::AtLeastOne(binding) => Self::AtLeastOne(binding.clone()),
            Self::Any(binding) => Self::Any(binding.clone()),
        }
    }
}

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync,
    BindingAny<Id, Ctx>: From<Self>,
{
    /// Erases the binding cardinality.
    pub fn into_any(self) -> BindingAny<Id, Ctx> {
        self.into()
    }
}

/// A component and context configured for the async-capable runtime.
#[must_use = "call .instantiate().await or .link().await to create a PluginInstance"]
pub struct Plugin<Ctx: 'static>(crate::plugin::Plugin<Ctx>);

impl<Ctx> Plugin<Ctx>
where
    Ctx: PluginContext + 'static,
{
    /// Creates an async-capable plugin declaration.
    pub fn new(component: Component, context: Ctx) -> Self {
        Self(crate::plugin::Plugin::new(component, context))
    }

    /// Sets fuel available during instantiation.
    pub fn with_initial_fuel(mut self, fuel: u64) -> Self {
        self.0 = self.0.with_initial_fuel(fuel);
        self
    }

    /// Sets the per-call fuel limiter.
    pub fn with_fuel_limiter(
        mut self,
        mut limiter: impl FnMut(&mut Store<Ctx>, &str, &str, &Function) -> u64 + Send + 'static,
    ) -> Self {
        self.0 = self
            .0
            .with_fuel_limiter(move |store, interface, name, metadata| {
                let function = match metadata.is_async() {
                    true => Function::Async(crate::Function::from_metadata(metadata)),
                    false => Function::Sync(crate::Function::from_metadata(metadata)),
                };
                limiter(store, interface, name, &function)
            });
        self
    }

    /// Sets the per-call epoch deadline limiter.
    pub fn with_epoch_limiter(
        mut self,
        mut limiter: impl FnMut(&mut Store<Ctx>, &str, &str, &Function) -> u64 + Send + 'static,
    ) -> Self {
        self.0 = self
            .0
            .with_epoch_limiter(move |store, interface, name, metadata| {
                let function = match metadata.is_async() {
                    true => Function::Async(crate::Function::from_metadata(metadata)),
                    false => Function::Sync(crate::Function::from_metadata(metadata)),
                };
                limiter(store, interface, name, &function)
            });
        self
    }

    /// Installs a Wasmtime memory/table limiter from the context.
    pub fn with_memory_limiter(
        mut self,
        limiter: impl (FnMut(&mut Ctx) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync + 'static,
    ) -> Self {
        self.0 = self.0.with_memory_limiter(limiter);
        self
    }

    /// Remaps requested interfaces to component exports.
    pub fn remap_interfaces(mut self, remaps: HashMap<String, Remap>) -> Self {
        self.0 = self.0.remap_interfaces(remaps);
        self
    }

    /// Links socket bindings and instantiates the plugin.
    ///
    /// # Errors
    ///
    /// Returns an error when linking, validation, or instantiation fails.
    pub async fn link<Id, Sockets, Executor>(
        self,
        engine: &Engine,
        linker: Linker<Ctx>,
        sockets: Sockets,
        executor: Executor,
    ) -> Result<PluginInstanceAsync<Ctx>, wasmtime::Error>
    where
        Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
        Sockets: IntoIterator,
        Sockets::Item: Into<BindingAny<Id, Ctx>>,
        Executor: Spawn + Send + Sync + 'static,
    {
        self.0
            .link_async(
                engine,
                linker,
                sockets
                    .into_iter()
                    .map(|binding| binding.into().into_core()),
                executor,
            )
            .await
    }

    /// Instantiates a plugin with no socket bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when asynchronous instantiation fails.
    pub async fn instantiate<Executor>(
        self,
        engine: &Engine,
        linker: &Linker<Ctx>,
        executor: Executor,
    ) -> Result<PluginInstanceAsync<Ctx>, wasmtime::Error>
    where
        Executor: Spawn + Send + Sync + 'static,
    {
        self.0.instantiate_async(engine, linker, executor).await
    }
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
