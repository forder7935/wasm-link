use crate::cardinality::{Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne};
use crate::plugin_instance::DispatchError as CoreDispatchError;
use crate::plugin_instance::PluginInstanceSync;
use crate::{DispatchError, Interface, PluginContext};
use std::collections::HashMap;
use std::sync::Arc;
use wasmtime::component::Val;

// The aliases below keep the otherwise repetitive cardinality projections local.
type PluginSockets<Id, Ctx, Plugins> =
    <Plugins as Cardinality<Id, PluginInstanceSync<Ctx>>>::Rebind<Arc<PluginInstanceSync<Ctx>>>;
type Results<Id, Ctx, Plugins> = <PluginSockets<Id, Ctx, Plugins> as Cardinality<
    Id,
    Arc<PluginInstanceSync<Ctx>>,
>>::Rebind<Result<Val, DispatchError>>;
type CoreResults<Id, Ctx, Plugins> = <PluginSockets<Id, Ctx, Plugins> as Cardinality<
    Id,
    Arc<PluginInstanceSync<Ctx>>,
>>::Rebind<Result<Val, CoreDispatchError>>;

/// A binding in the synchronous runtime.
pub struct Binding<Id, Ctx, Plugins = ExactlyOne<Id, PluginInstanceSync<Ctx>>>(
    pub(crate) crate::binding::Binding<Id, Ctx, Plugins, PluginInstanceSync<Ctx>>,
)
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync;

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Cardinality<Id, Arc<PluginInstanceSync<Ctx>>> + Send + Sync,
    CoreResults<Id, Ctx, Plugins>: Cardinality<
        Id,
        Result<Val, CoreDispatchError>,
        Rebind<Result<Val, DispatchError>> = Results<Id, Ctx, Plugins>,
    >,
{
    /// Creates a synchronous binding.
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

    /// Dispatches a synchronous call.
    ///
    /// # Errors
    ///
    /// Returns an error when the interface or function is unknown. Per-plugin
    /// execution errors are returned inside the binding's cardinality wrapper.
    pub fn dispatch(
        &self,
        interface: &str,
        function: &str,
        args: &[Val],
    ) -> Result<Results<Id, Ctx, Plugins>, DispatchError> {
        self.0
            .dispatch(interface, function, args)
            .map(|results| results.map_mut(|result| result.map_err(Into::into)))
            .map_err(Into::into)
    }
}

impl<Id, Ctx, Plugins> Clone for Binding<Id, Ctx, Plugins>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
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
    Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync + std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A synchronous binding with erased cardinality.
#[derive(Debug)]
pub enum BindingAny<Id, Ctx>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    /// Exactly one implementation.
    ExactlyOne(Binding<Id, Ctx, ExactlyOne<Id, PluginInstanceSync<Ctx>>>),
    /// Zero or one implementation.
    AtMostOne(Binding<Id, Ctx, AtMostOne<Id, PluginInstanceSync<Ctx>>>),
    /// One or more implementations.
    AtLeastOne(Binding<Id, Ctx, AtLeastOne<Id, PluginInstanceSync<Ctx>>>),
    /// Any number of implementations.
    Any(Binding<Id, Ctx, Any<Id, PluginInstanceSync<Ctx>>>),
}

impl<Id, Ctx> BindingAny<Id, Ctx>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    pub(crate) fn into_core(self) -> crate::binding::BindingAny<Id, Ctx, PluginInstanceSync<Ctx>> {
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
        impl<Id, Ctx> From<Binding<Id, Ctx, $cardinality<Id, PluginInstanceSync<Ctx>>>>
            for BindingAny<Id, Ctx>
        where
            Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
            Ctx: PluginContext + 'static,
        {
            fn from(binding: Binding<Id, Ctx, $cardinality<Id, PluginInstanceSync<Ctx>>>) -> Self {
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
    Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
    PluginSockets<Id, Ctx, Plugins>: Send + Sync,
    BindingAny<Id, Ctx>: From<Self>,
{
    /// Erases the binding cardinality.
    pub fn into_any(self) -> BindingAny<Id, Ctx> {
        self.into()
    }
}
