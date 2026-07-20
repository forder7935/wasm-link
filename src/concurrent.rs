//! Concurrent, async-capable plugin runtime.
//!
//! Methods mirror [`crate::sync`]; runtime operations return futures and must be
//! awaited. Runtime-state types cannot be mixed across the two modules.

use std::collections::{ HashMap, HashSet };
use futures::task::Spawn;
use wasmtime::component::{ Component, Linker, Val };
use wasmtime::{ Engine, Store };

use crate::cardinality::Cardinality;
use crate::interface::{ Function as FunctionMetadata, Interface as InterfaceMetadata };
use crate::plugin::Plugin as PluginCore;
use crate::plugin_instance::PluginInstanceAsync;
use crate::runtime_binding::define_runtime_bindings;
use crate::{ DispatchError, FunctionKind, PluginContext, Remap, ReturnKind };

/// An instantiated plugin in the concurrent runtime.
pub use crate::plugin_instance::PluginInstanceAsync as PluginInstance;

/// Metadata for a WIT function in the concurrent runtime.
#[derive( Debug, Clone )]
pub struct Function {
	kind: FunctionKind,
	return_kind: ReturnKind,
	is_async: bool,
}

impl Function {
	/// Creates metadata for a synchronous WIT function.
	pub fn new( kind: FunctionKind, return_kind: ReturnKind ) -> Self {
		Self { kind, return_kind, is_async: false }
	}

	/// Creates metadata for a WIT function declared with the `async` effect.
	pub fn new_async( kind: FunctionKind, return_kind: ReturnKind ) -> Self {
		Self { kind, return_kind, is_async: true }
	}

	/// Returns whether the function is freestanding or a resource method.
	pub fn kind( &self ) -> FunctionKind { self.kind }

	/// Returns how dispatch handles the function's return value.
	pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

	/// Returns whether the WIT function has the `async` effect.
	pub fn is_async( &self ) -> bool { self.is_async }

	fn into_metadata( self ) -> FunctionMetadata {
		match self.is_async {
			true => FunctionMetadata::new_async( self.kind, self.return_kind ),
			false => FunctionMetadata::new( self.kind, self.return_kind ),
		}
	}

	fn from_metadata( metadata: &FunctionMetadata ) -> Self {
		Self {
			kind: metadata.kind(),
			return_kind: metadata.return_kind(),
			is_async: metadata.is_async(),
		}
	}
}

/// A WIT interface declaration in the concurrent runtime.
#[derive( Debug, Clone, Default )]
pub struct Interface {
	functions: HashMap<String, Function>,
	resources: HashSet<String>,
}

impl Interface {
	/// Creates an interface containing synchronous or asynchronous functions.
	pub fn new( functions: HashMap<String, Function>, resources: HashSet<String> ) -> Self {
		Self { functions, resources }
	}

	fn into_metadata( self ) -> InterfaceMetadata {
		InterfaceMetadata::new(
			self.functions.into_iter()
				.map(|( name, function )| ( name, function.into_metadata() ))
				.collect(),
			self.resources,
		)
	}
}

define_runtime_bindings!(
	PluginInstanceAsync,
	"A binding in the concurrent runtime.",
	"A concurrent binding with erased cardinality."
);

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceAsync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Cardinality<
		Id,
		std::sync::Arc<futures::lock::Mutex<PluginInstanceAsync<Ctx>>>,
	> + Send + Sync,
{
	/// Dispatches a function call.
	///
	/// # Errors
	///
	/// Returns an error if the interface or function is not declared by the binding.
	pub async fn dispatch(
		&self,
		interface_name: &str,
		function_name: &str,
		args: &[Val],
	) -> Result<Results<Id, Ctx, Plugins>, DispatchError>
	where
		Results<Id, Ctx, Plugins>: Send,
	{
		self.0.dispatch_async( interface_name, function_name, args ).await
	}
}

/// A component and context configured for the concurrent runtime.
#[must_use = "call .instantiate().await or .link().await to create a PluginInstance"]
pub struct Plugin<Ctx: 'static>( PluginCore<Ctx> );

impl<Ctx> Plugin<Ctx>
where
	Ctx: PluginContext + 'static,
{
	/// Creates a plugin declaration.
	pub fn new( component: Component, context: Ctx ) -> Self {
		Self( PluginCore::new( component, context ))
	}

	/// Sets the fuel available during instantiation.
	pub fn with_initial_fuel( mut self, fuel: u64 ) -> Self {
		self.0 = self.0.with_initial_fuel( fuel );
		self
	}

	/// Sets the per-call fuel limiter.
	pub fn with_fuel_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_fuel_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &Function::from_metadata( function ))
		});
		self
	}

	/// Sets the per-call epoch deadline limiter.
	pub fn with_epoch_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_epoch_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &Function::from_metadata( function ))
		});
		self
	}

	/// Installs a Wasmtime memory/table limiter from the context.
	pub fn with_memory_limiter(
		mut self,
		limiter: impl (FnMut( &mut Ctx ) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync + 'static,
	) -> Self {
		self.0 = self.0.with_memory_limiter( limiter );
		self
	}

	/// Remaps requested interfaces to component exports.
	pub fn remap_interfaces( mut self, remaps: HashMap<String, Remap> ) -> Self {
		self.0 = self.0.remap_interfaces( remaps );
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
	) -> Result<PluginInstance<Ctx>, wasmtime::Error>
	where
		Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<Id, Ctx>>,
		Executor: Spawn + Send + Sync + 'static,
	{
		self.0.link_async(
			engine,
			linker,
			sockets.into_iter().map(| binding | binding.into().into_core()),
			executor,
		).await
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
	) -> Result<PluginInstance<Ctx>, wasmtime::Error>
	where
		Executor: Spawn + Send + Sync + 'static,
	{
		self.0.instantiate_async( engine, linker, executor ).await
	}
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
	fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		self.0.fmt( formatter )
	}
}
