//! Plugin metadata types.
//!
//! A plugin is a WASM component that implements one [`Binding`]( crate::Binding )
//! (its **plug**) and may depend on zero or more other [`Binding`]( crate::Binding )s
//! (its **sockets**). The plug declares what the plugin exports; sockets declare what
//! the plugin expects to import from other plugins.

use wasmtime::{ Engine, Store };
use wasmtime::component::{ Component, ResourceTable, Linker, Val };

use crate::Binding ;
use crate::plugin_instance::PluginInstance ;
use crate::Function ;

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
pub struct Plugin<Ctx: 'static> {
    /// Compiled WASM component
    component: Component,
    /// User context consumed at load time to become `Store<Ctx>`
    context: Ctx,
    /// Closure that determines fuel for each function call
    #[allow( clippy::type_complexity )]
    fuel_limiter: Option<Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>>,
    /// Closure that determines epoch deadline for each function call
    #[allow( clippy::type_complexity )]
    epoch_limiter: Option<Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>>,
    /// Closure that returns a mutable reference to the `ResourceLimiter` in the context
    #[allow( clippy::type_complexity )]
    memory_limiter: Option<Box<dyn (FnMut( &mut Ctx ) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync>>,
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
            fuel_limiter: None,
            epoch_limiter: None,
            memory_limiter: None,
        }
    }

    /// Sets a closure that determines the fuel limit for each function call.
    ///
    /// The closure receives the store, the interface path (e.g., `"my:package/api"`),
    /// the function name, and the [`Function`] metadata. It returns the fuel to set.
    ///
    /// **Warning:** Fuel consumption must be enabled in the [`Engine`]( wasmtime::Engine )
    /// via [`Config::consume_fuel`]( wasmtime::Config::consume_fuel ). If not enabled,
    /// dispatch will fail with a [`RuntimeException`]( crate::DispatchError::RuntimeException )
    /// at call time.
    ///
    /// ```
    /// # use wasm_link::{ Plugin, PluginContext, ResourceTable, Component, Engine };
    /// # struct Ctx { resource_table: ResourceTable }
    /// # impl PluginContext for Ctx {
    /// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// # }
    /// # fn example( component: Component ) {
    /// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
    ///     .with_fuel_limiter(| _store, _interface, _function, _metadata | 100_000 );
    /// # }
    /// ```
    pub fn with_fuel_limiter( mut self, limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static ) -> Self {
        self.fuel_limiter = Some( Box::new( limiter ));
        self
    }

    /// Sets a closure that determines the epoch deadline for each function call.
    ///
    /// The closure receives the store, the interface path (e.g., `"my:package/api"`),
    /// the function name, and the [`Function`] metadata. It returns the epoch deadline
    /// in ticks.
    ///
    /// **Warning:** Epoch interruption must be enabled in the [`Engine`]( wasmtime::Engine )
    /// via [`Config::epoch_interruption`]( wasmtime::Config::epoch_interruption ). If not
    /// enabled, the deadline is silently ignored.
    ///
    /// ```
    /// # use wasm_link::{ Plugin, PluginContext, ResourceTable, Component, Engine };
    /// # struct Ctx { resource_table: ResourceTable }
    /// # impl PluginContext for Ctx {
    /// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// # }
    /// # fn example( component: Component ) {
    /// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
    ///     .with_epoch_limiter(| _store, _interface, _function, _metadata | 5 );
    /// # }
    /// ```
    pub fn with_epoch_limiter( mut self, limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static ) -> Self {
        self.epoch_limiter = Some( Box::new( limiter ));
        self
    }

    /// Sets a closure that returns a mutable reference to a [`ResourceLimiter`]( wasmtime::ResourceLimiter )
    /// embedded in the plugin context.
    ///
    /// The limiter is installed into the wasmtime [`Store`]( wasmtime::Store ) once at instantiation
    /// and controls memory and table growth for the lifetime of the plugin.
    ///
    /// The [`ResourceLimiter`]( wasmtime::ResourceLimiter ) must be stored inside the context type `Ctx`
    /// so that wasmtime can access it through a `&mut Ctx` reference.
    ///
    /// ```
    /// # use wasm_link::{ Plugin, PluginContext, ResourceTable, Component, Engine };
    /// # struct Ctx { resource_table: ResourceTable, limiter: MyLimiter }
    /// # impl PluginContext for Ctx {
    /// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// # }
    /// # struct MyLimiter;
    /// # impl wasmtime::ResourceLimiter for MyLimiter {
    /// #     fn memory_growing( &mut self, _: usize, _: usize, _: Option<usize> ) -> anyhow::Result<bool> { anyhow::Ok( true ) }
    /// #     fn table_growing( &mut self, _: usize, _: usize, _: Option<usize> ) -> anyhow::Result<bool> { anyhow::Ok( true ) }
    /// # }
    /// # fn example( component: Component ) {
    /// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new(), limiter: MyLimiter })
    ///     .with_memory_limiter(| ctx | &mut ctx.limiter );
    /// # }
    /// ```
    pub fn with_memory_limiter(
        mut self,
        limiter: impl (FnMut( &mut Ctx ) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync + 'static,
    ) -> Self {
        self.memory_limiter = Some( Box::new( limiter ));
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
        if let Some( limiter ) = self.memory_limiter { store.limiter( limiter ); }
        let instance = linker.instantiate( &mut store, &self.component )?;
        Ok( PluginInstance {
            store,
            instance,
            fuel_limiter: self.fuel_limiter,
            epoch_limiter: self.epoch_limiter,
        })
    }

}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Plugin" )
            .field( "component", &"<Component>" )
            .field( "context", &self.context )
            .field( "fuel_limiter", &self.fuel_limiter.as_ref().map(| _ | "<closure>" ))
            .field( "epoch_limiter", &self.epoch_limiter.as_ref().map(| _ | "<closure>" ))
            .field( "memory_limiter", &self.memory_limiter.as_ref().map(| _ | "<closure>" ))
            .finish_non_exhaustive()
    }
}
