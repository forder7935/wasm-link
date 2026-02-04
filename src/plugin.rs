//! Plugin metadata types.
//!
//! A plugin is a WASM component that implements one [`Binding`]( crate::Binding )
//! (its **plug**) and may depend on zero or more other [`Binding`]( crate::Binding )s
//! (its **sockets**). The plug declares what the plugin exports; sockets declare what
//! the plugin expects to import from other plugins.

use wasmtime::component::{ Component, ResourceTable } ;

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

/// A plugin declaration with its WASM component and runtime context.
///
/// Each plugin declares:
/// - A **plug**: the [`Binding`]( crate::Binding ) it implements (what it exports)
/// - Zero or more **sockets**: the [`Binding`]( crate::Binding )s it depends on (what it imports)
/// - A **component**: the compiled WASM component
/// - A **context**: user data that will be provided to any host-exported function (`T` of `Store<T>`)
///
/// The `context` field is consumed during loading and becomes the wasmtime Store's data.
/// The Plugin struct itself is not retained after loading.
///
/// # Type Parameters
/// - `PluginId`: Unique identifier type for the plugin
/// - `BindingId`: [`Binding`]( crate::Binding ) identifier type (must match the IDs used in [`Binding`]( crate::Binding ))
/// - `Ctx`: User context type that will be stored in the wasmtime Store
pub struct Plugin<PluginId, BindingId, Ctx> {
    /// Unique identifier for this plugin
    id: PluginId,
    /// The binding this plugin implements
    plug: BindingId,
    /// Bindings this plugin depends on
    sockets: Vec<BindingId>,
    /// Compiled WASM component
    component: Component,
    /// User context consumed at load time to become `Store<Ctx>`
    context: Ctx,
}

impl<PluginId, BindingId, Ctx> Plugin<PluginId, BindingId, Ctx> {
    /// Creates a new plugin declaration.
    #[inline]
    pub fn new(
        id: PluginId,
        plug: BindingId,
        sockets: impl IntoIterator<Item = BindingId>,
        component: Component,
        context: Ctx,
    ) -> Self {
        Self {
            id,
            plug,
            sockets: sockets.into_iter().collect(),
            component,
            context,
        }
    }

    /// Unique identifier for this plugin.
    #[inline] pub fn id( &self ) -> &PluginId { &self.id }

    /// The binding this plugin implements.
    #[inline] pub fn plug( &self ) -> &BindingId { &self.plug }

    /// Bindings this plugin depends on.
    #[inline] pub fn sockets( &self ) -> &[BindingId] { &self.sockets }

    /// The compiled WASM component.
    #[inline] pub fn component( &self ) -> &Component { &self.component }

    /// Consumes the plugin, returning its parts for loading.
    ///
    /// Returns `(id, sockets, component, context)`. The `plug` field is excluded
    /// as it's only needed during plugin tree construction, not loading.
    #[inline]
    pub(crate) fn into_parts( self ) -> ( PluginId, Vec<BindingId>, Component, Ctx ) {
        ( self.id, self.sockets, self.component, self.context )
    }
}

impl<PluginId: std::fmt::Debug, BindingId: std::fmt::Debug, Ctx> std::fmt::Debug for Plugin<PluginId, BindingId, Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Plugin" )
            .field( "id", &self.id )
            .field( "plug", &self.plug )
            .field( "sockets", &self.sockets )
            .field( "component", &"<Component>" )
            .finish_non_exhaustive()
    }
}
