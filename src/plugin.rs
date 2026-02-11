//! Plugin metadata types.
//!
//! A plugin is a WASM component that implements one [`Binding`]( crate::Binding )
//! (its **plug**) and may depend on zero or more other [`Binding`]( crate::Binding )s
//! (its **sockets**). The plug declares what the plugin exports; sockets declare what
//! the plugin expects to import from other plugins.

use std::collections::HashMap ;
use wasmtime::{ Engine, Store };
use wasmtime::component::{ Component, ResourceTable, Linker, Val };

use crate::Binding ;
use crate::plugin_instance::PluginInstance ;

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

/// A WASM component bundled with its runtime context, ready for instantiation.
///
/// The component's exports (its **plug**) and imports (its **sockets**) are defined through
/// the [`Binding`], not by this struct.
///
/// The `context` is consumed during linking to become the wasmtime `Store`'s data.
///
/// # Type Parameters
/// - `Ctx`: User context type that will be stored in the wasmtime Store
#[must_use = "call .instantiate() or .link() to create a PluginInstance"]
pub struct Plugin<Ctx> {
    /// Compiled WASM component
    component: Component,
    /// User context consumed at load time to become `Store<Ctx>`
    context: Ctx,
    /// Multiplier applied to base fuel values
    fuel_multiplier: Option<f64>,
    /// Multiplier applied to base epoch deadline values
    epoch_deadline_multiplier: Option<f64>,
    /// Per-function fuel overrides: `Map<interface, Map<function, fuel>>`
    fuel_overrides: HashMap<String, HashMap<String, u64>>,
    /// Per-function epoch deadline overrides: `Map<interface, Map<function, ticks>>`
    epoch_deadline_overrides: HashMap<String, HashMap<String, u64>>,
}

impl<Ctx> Plugin<Ctx>
where
    Ctx: PluginContext + 'static,
{

    /// Creates a new plugin declaration.
    ///
    /// Note that the plugin ID is not specified here - it's provided when constructing
    /// the [`Socket`]( crate::socket::Socket ) that holds this plugin. This is done to prevent duplicate ids.
    pub fn new(
        component: Component,
        context: Ctx,
    ) -> Self {
        Self {
            component,
            context,
            fuel_multiplier: None,
            epoch_deadline_multiplier: None,
            fuel_overrides: HashMap::with_capacity( 0 ),
            epoch_deadline_overrides: HashMap::with_capacity( 0 ),
        }
    }

    /// Sets a multiplier applied to base fuel values.
    ///
    /// The multiplier scales the fuel limit from either the function or binding default.
    ///
    /// # Quirks
    ///
    /// - **Zero or negative** → treated as 0 (immediate fuel exhaustion)
    /// - **Integer values** (1.0, 2.0, etc.) → exact multiplication, no precision loss
    /// - **Non-integer values with base > 2^53** → base is truncated to fit f64's
    ///   mantissa; least significant bits are lost
    ///
    /// ```
    /// # use wasm_link::{ Plugin, PluginContext, ResourceTable, Component, Engine };
    /// # struct Ctx { resource_table: ResourceTable }
    /// # impl PluginContext for Ctx {
    /// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// # }
    /// # fn example( component: Component ) {
    /// // Untrusted plugin gets half the normal fuel budget
    /// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
    ///     .with_fuel_multiplier( 0.5 );
    /// # }
    /// ```
    pub fn with_fuel_multiplier( mut self, multiplier: f64 ) -> Self {
        self.fuel_multiplier = Some( multiplier );
        self
    }

    /// Sets a multiplier applied to base epoch deadline values.
    ///
    /// The multiplier scales the epoch deadline from either the function or binding default.
    ///
    /// # Quirks
    ///
    /// - **Zero or negative** → treated as 0 (immediate deadline)
    /// - **Integer values** (1.0, 2.0, etc.) → exact multiplication, no precision loss
    /// - **Non-integer values with base > 2^53** → base is truncated to fit into f64's
    ///   mantissa; least significant bits are lost
    pub fn with_epoch_deadline_multiplier( mut self, multiplier: f64 ) -> Self {
        self.epoch_deadline_multiplier = Some( multiplier );
        self
    }

    /// Sets per-function fuel overrides for this plugin.
    ///
    /// The outer map is keyed by interface path (e.g., `"my:package/interface"`),
    /// the inner map by function name. These overrides take the highest precedence,
    /// bypassing both multipliers and function/binding defaults.
    ///
    /// **Note:** Invalid keys (non-existent interface/function pairs) are silently
    /// ignored at dispatch time. Double-check your keys to avoid typos.
    ///
    /// ```
    /// # use std::collections::HashMap;
    /// # use wasm_link::{ Plugin, PluginContext, ResourceTable, Component };
    /// # struct Ctx { resource_table: ResourceTable }
    /// # impl PluginContext for Ctx {
    /// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// # }
    /// # fn example( component: Component ) {
    /// // Give the "compute" function extra fuel regardless of defaults
    /// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
    ///     .with_fuel_overrides( HashMap::from([
    ///         ( "my:pkg/math".into(), HashMap::from([( "compute".into(), 1_000_000 )])),
    ///     ]));
    /// # }
    /// ```
    pub fn with_fuel_overrides( mut self, overrides: HashMap<String, HashMap<String, u64>> ) -> Self {
        self.fuel_overrides = overrides ;
        self
    }

    /// Sets per-function epoch deadline overrides for this plugin.
    ///
    /// The outer map is keyed by interface path (e.g., `"my:package/interface"`),
    /// the inner map by function name. These overrides take the highest precedence,
    /// bypassing both multipliers and function/binding defaults.
    ///
    /// **Note:** Invalid keys (non-existent interface/function pairs) are silently
    /// ignored at dispatch time. Double-check your keys to avoid typos.
    pub fn with_epoch_deadline_overrides( mut self, overrides: HashMap<String, HashMap<String, u64>> ) -> Self {
        self.epoch_deadline_overrides = overrides ;
        self
    }

    /// Links this plugin with its socket bindings and instantiates it.
    ///
    /// Takes ownership of the `linker` because socket bindings are added to it. If you need
    /// to reuse the same linker for multiple plugins, clone it before passing it in.
    ///
    /// # Type Parameters
    /// - `PluginId`: Must implement `Into<Val>` so plugin IDs can be passed to WASM when
    ///   dispatching to multi-plugin sockets (the ID identifies which plugin produced each result).
    ///
    /// # Errors
    /// Returns an error if linking or instantiation fails.
    pub fn link<PluginId>(
        self,
        engine: &Engine,
        mut linker: Linker<Ctx>,
        sockets: impl IntoIterator<Item = Binding<PluginId, Ctx>>,
    ) -> Result<PluginInstance<Ctx>, wasmtime::Error>
    where
        PluginId: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
    {
        sockets.into_iter().try_for_each(| binding | Binding::add_to_linker( &binding, &mut linker ))?;
        Self::instantiate( self, engine, &linker )
    }

    /// A convenience alias for [`Plugin::link`] with 0 sockets
    ///
    /// # Errors
    /// Returns an error if instantiation fails.
    pub fn instantiate(
        self,
        engine: &Engine,
        linker: &Linker<Ctx>
    ) -> Result<PluginInstance<Ctx>, wasmtime::Error> {
        let mut store = Store::new( engine, self.context );
        let instance = linker.instantiate( &mut store, &self.component )?;
        Ok( PluginInstance {
            store,
            instance,
            fuel_multiplier: self.fuel_multiplier,
            epoch_deadline_multiplier: self.epoch_deadline_multiplier,
            fuel_overrides: self.fuel_overrides,
            epoch_deadline_overrides: self.epoch_deadline_overrides,
        })
    }

}

impl<Ctx: std::fmt::Debug> std::fmt::Debug for Plugin<Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Plugin" )
            .field( "component", &"<Component>" )
            .field( "context", &self.context )
            .field( "fuel_multiplier", &self.fuel_multiplier )
            .field( "epoch_deadline_multiplier", &self.epoch_deadline_multiplier )
            .field( "fuel_overrides", &self.fuel_overrides )
            .field( "epoch_deadline_overrides", &self.epoch_deadline_overrides )
            .finish_non_exhaustive()
    }
}
