use std::collections::HashMap;
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Engine, Store};

use crate::plugin_instance::PluginInstanceSync;
use crate::{BindingAny, Function, PluginContext, Remap};

/// A component and context configured for the synchronous runtime.
#[must_use = "call .instantiate() or .link() to create a PluginInstance"]
pub struct Plugin<Ctx: 'static>(crate::plugin::Plugin<Ctx>);

impl<Ctx> Plugin<Ctx>
where
    Ctx: PluginContext + 'static,
{
    /// Creates a synchronous plugin declaration.
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
            .with_fuel_limiter(move |store, interface, name, function| {
                limiter(store, interface, name, &Function::from_metadata(function))
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
            .with_epoch_limiter(move |store, interface, name, function| {
                limiter(store, interface, name, &Function::from_metadata(function))
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

    /// Links synchronous socket bindings and instantiates the plugin.
    ///
    /// # Errors
    ///
    /// Returns an error when linking, validation, or instantiation fails.
    pub fn link<Id, Sockets>(
        self,
        engine: &Engine,
        linker: Linker<Ctx>,
        sockets: Sockets,
    ) -> Result<PluginInstanceSync<Ctx>, wasmtime::Error>
    where
        Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
        Sockets: IntoIterator,
        Sockets::Item: Into<BindingAny<Id, Ctx>>,
    {
        self.0.link(
            engine,
            linker,
            sockets
                .into_iter()
                .map(|binding| binding.into().into_core()),
        )
    }

    /// Instantiates a plugin with no socket bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when the component contains WIT-async functions or
    /// synchronous instantiation fails.
    pub fn instantiate(
        self,
        engine: &Engine,
        linker: &Linker<Ctx>,
    ) -> Result<PluginInstanceSync<Ctx>, wasmtime::Error> {
        self.0.instantiate(engine, linker)
    }
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
