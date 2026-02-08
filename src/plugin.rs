//! Plugin metadata types.
//!
//! A plugin is a WASM component that implements one [`Binding`]( crate::Binding )
//! (its **plug**) and may depend on zero or more other [`Binding`]( crate::Binding )s
//! (its **sockets**). The plug declares what the plugin exports; sockets declare what
//! the plugin expects to import from other plugins.

use wasmtime::{ Engine, Store };
use wasmtime::component::{ Component, ResourceTable, Linker, Val };

use crate::Binding ;
use crate::plugin_instance::PluginInstance;

/// Trait for accessing a [`ResourceTable`] from the store's data type.
///
/// Resources that flow between plugins need to be wrapped to track ownership.
/// This trait provides access to the table where those wrapped resources are stored.
///
/// # Example
///
/// ```
/// use wasmtime::component::ResourceTable ;
/// use wasm_link::PluginContext ;
///
/// struct MyPluginData {
///     resource_table: ResourceTable,
///     // ... other fields
/// }
///
/// impl PluginContext for MyPluginData {
///     fn resource_table( &mut self ) -> &mut ResourceTable {
///         &mut self.resource_table
///     }
/// }
/// ```
pub trait PluginContext: Send {
    /// Returns a mutable reference to a resource table.
    fn resource_table( &mut self ) -> &mut ResourceTable ;
}

/// A WASM component bundled with its runtime context.
///
/// The component's exports (its **plug**) and imports (its **sockets**) are defined through
/// the [`Binding`], not by this struct.
///
/// The `context` is consumed during linking to become the wasmtime `Store`'s data.
///
/// # Type Parameters
/// - `Ctx`: User context type that will be stored in the wasmtime Store
pub struct Plugin<Ctx> {
    /// Compiled WASM component
    component: Component,
    /// User context consumed at load time to become `Store<Ctx>`
    context: Ctx,
}

impl<Ctx> Plugin<Ctx>
where
    Ctx: PluginContext + 'static,
{

    /// Creates a new plugin declaration.
    ///
    /// Note that the plugin ID is not specified here - it's provided when constructing
    /// the [`Socket`]( crate::socket::Socket ) that holds this plugin. This is done to prevent duplicate ids.
    #[inline]
    pub fn new(
        component: Component,
        context: Ctx,
    ) -> Self {
        Self { component, context }
    }

    /// Links this plugin with its socket bindings and instantiates it.
    ///
    /// # Errors
    /// Returns an error if linking or instantiation fails.
    #[inline] pub fn link<PluginId>(
        self,
        engine: &Engine,
        mut linker: Linker<Ctx>,
        sockets: impl IntoIterator<Item = Binding<PluginId, Ctx>>,
    ) -> Result<PluginInstance<Ctx>, wasmtime::Error>
    where
        PluginId: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
    {
        sockets.into_iter().try_for_each(| binding | Binding::add_to_linker( &binding, &mut linker ))?;
        let mut store = Store::new( engine, self.context );
        let instance = linker.instantiate( &mut store, &self.component )?;
        Ok( PluginInstance { store, instance })
    }

    /// A convenience alias for [`Plugin::link`] with 0 sockets
    ///
    /// # Errors
    /// Returns an error if instantiation fails.
    #[inline] pub fn instantiate(
        self,
        engine: &Engine,
        linker: &Linker<Ctx>
    ) -> Result<PluginInstance<Ctx>, wasmtime::Error> {
        let mut store = Store::new( engine, self.context );
        let instance = linker.instantiate( &mut store, &self.component )?;
        Ok( PluginInstance { store, instance })
    }

}

impl<Ctx: std::fmt::Debug> std::fmt::Debug for Plugin<Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Plugin" )
            .field( "component", &"<Component>" )
            .field( "context", &self.context )
            .finish_non_exhaustive()
    }
}
