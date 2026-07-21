//! Plugin metadata types.
//!
//! A plugin is a WASM component that implements one [`Binding`]( crate::Binding )
//! (its **plug**) and may depend on zero or more other [`Binding`]( crate::Binding )s
//! (its **sockets**). The plug declares what the plugin exports; sockets declare what
//! the plugin expects to import from other plugins.

use std::collections::HashMap ;
use wasmtime::{ Engine, Store };
use wasmtime::component::{ Component, ResourceTable, Linker, Val };
use wasmtime::component::types::{ Component as ComponentType, ComponentInstance, ComponentItem };
use futures::task::Spawn ;

use crate::binding::BindingAny ;
use crate::plugin_instance::{ concurrent, sync };
use crate::interface::FunctionMetadata as Function ;
use crate::Remap ;

/// Trait for accessing a [`ResourceTable`] from the store's data type.
///
/// Resources that flow between plugins need to be wrapped to track ownership.
/// This trait provides access to the table where those wrapped resources are stored.
/// [`ResourceTable`] is part of the wasmtime component model; see the
/// [wasmtime docs](https://docs.rs/wasmtime/latest/wasmtime/component/) for details.
///
/// # Example
///
/// ```
/// use wasmtime::component::ResourceTable ;
/// use wasm_link::PluginContext ;
///
/// struct MyPluginData {
/// 	resource_table: ResourceTable,
/// 	// ... other fields
/// }
///
/// impl PluginContext for MyPluginData {
/// 	fn resource_table( &mut self ) -> &mut ResourceTable {
/// 		&mut self.resource_table
/// 	}
/// }
/// ```
pub trait PluginContext: Send {
	/// Returns a mutable reference to a resource table.
	fn resource_table( &mut self ) -> &mut ResourceTable ;
}

/// A WASM component bundled with its runtime context, ready for instantiation.
///
/// The component's exports (its **plug**) and imports (its **sockets**) are defined through
/// the [`crate::Binding`], not by this struct.
///
/// The `context` is consumed during linking to become the wasmtime [`Store`]( wasmtime::Store )'s data.
///
/// # Type Parameters
/// - `Ctx`: User context type that will be stored in the wasmtime [`Store`]( wasmtime::Store )
///
/// # Example
///
/// ```
/// # use wasm_link::sync::Plugin;
/// # use wasm_link::{ PluginContext, ResourceTable, Component, Engine, Linker };
/// # struct Ctx { resource_table: ResourceTable }
/// # impl PluginContext for Ctx {
/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
/// # }
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let engine = Engine::default();
/// let linker = Linker::new( &engine );
///
/// let plugin = Plugin::new(
/// 	Component::new( &engine, "(component)" )?,
/// 	Ctx { resource_table: ResourceTable::new() },
/// ).instantiate( &engine, &linker )?;
/// # let _ = plugin;
/// # Ok(())
/// # }
/// ```
#[must_use = "call .instantiate() or .link() to create a plugin instance"]
pub struct Plugin<Ctx: 'static> {
	/// Compiled WASM component
	component: Component,
	/// User context consumed at load time to become `Store<Ctx>`
	context: Ctx,
	/// Per-interface export name remaps for this plugin
	interface_remaps: HashMap<String, Remap>,
	/// Fuel assigned to the store before component instantiation
	initial_fuel: Option<u64>,
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
	pub(crate) fn ensure_synchronous( &self, engine: &Engine ) -> Result<(), wasmtime::Error> {
		let component_type = self.component.component_type();
		match component_contains_async( engine, &component_type ) {
			true => Err( wasmtime::Error::msg( "synchronous plugins cannot contain WIT-async functions" )),
			false => Ok(()),
		}
	}

	/// Creates a new plugin declaration.
	///
	/// Note that the plugin ID is not specified here - it's provided when constructing
	/// the cardinality wrapper that holds this plugin. This is done to prevent duplicate ids.
	pub fn new(
		component: Component,
		context: Ctx,
	) -> Self {
		Self {
			component,
			context,
			interface_remaps: HashMap::new(),
			initial_fuel: None,
			fuel_limiter: None,
			epoch_limiter: None,
			memory_limiter: None,
		}
	}

	/// Sets the fuel available when component instantiation begins.
	///
	/// Instantiation can execute WebAssembly startup code, including complex global,
	/// element, table, and memory initializers and explicit start functions. Any fuel
	/// left after instantiation remains available to subsequent calls. A
	/// [`with_fuel_limiter`](Self::with_fuel_limiter) invocation may inspect or replace
	/// that remainder before a call.
	///
	/// **Warning:** Fuel consumption must be enabled in the [`Engine`]( wasmtime::Engine )
	/// via [`Config::consume_fuel`]( wasmtime::Config::consume_fuel ). If not enabled,
	/// instantiation will fail when the initial fuel is applied.
	///
	/// ```
	/// # use wasm_link::sync::Plugin;
	/// # use wasm_link::{ PluginContext, ResourceTable, Component };
	/// # struct Ctx { resource_table: ResourceTable }
	/// # impl PluginContext for Ctx {
	/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
	/// # }
	/// # fn example( component: Component ) {
	/// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
	/// 	.with_initial_fuel( 100_000 );
	/// # let _ = plugin;
	/// # }
	/// ```
	pub fn with_initial_fuel( mut self, fuel: u64 ) -> Self {
		self.initial_fuel = Some( fuel );
		self
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
	/// # use wasm_link::sync::Plugin;
	/// # use wasm_link::{ PluginContext, ResourceTable, Component, Engine };
	/// # struct Ctx { resource_table: ResourceTable }
	/// # impl PluginContext for Ctx {
	/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
	/// # }
	/// # fn example( component: Component ) {
	/// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
	/// 	.with_fuel_limiter(| _store, _interface, _function, _metadata | 100_000 );
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
	/// # use wasm_link::sync::Plugin;
	/// # use wasm_link::{ PluginContext, ResourceTable, Component, Engine };
	/// # struct Ctx { resource_table: ResourceTable }
	/// # impl PluginContext for Ctx {
	/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
	/// # }
	/// # fn example( component: Component ) {
	/// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new() })
	/// 	.with_epoch_limiter(| _store, _interface, _function, _metadata | 5 );
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
	/// # use wasm_link::sync::Plugin;
	/// # use wasm_link::{ PluginContext, ResourceTable, Component, Engine };
	/// # struct Ctx { resource_table: ResourceTable, limiter: MyLimiter }
	/// # impl PluginContext for Ctx {
	/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
	/// # }
	/// # struct MyLimiter;
	/// # impl wasmtime::ResourceLimiter for MyLimiter {
	/// # 	fn memory_growing( &mut self, _: usize, _: usize, _: Option<usize> ) -> wasmtime::Result<bool> { Ok( true ) }
	/// # 	fn table_growing( &mut self, _: usize, _: usize, _: Option<usize> ) -> wasmtime::Result<bool> { Ok( true ) }
	/// # }
	/// # fn example( component: Component ) {
	/// let plugin = Plugin::new( component, Ctx { resource_table: ResourceTable::new(), limiter: MyLimiter })
	/// 	.with_memory_limiter(| ctx | &mut ctx.limiter );
	/// # }
	/// ```
	pub fn with_memory_limiter(
		mut self,
		limiter: impl (FnMut( &mut Ctx ) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync + 'static,
	) -> Self {
		self.memory_limiter = Some( Box::new( limiter ));
		self
	}

	/// Sets interface export remaps for this plugin.
	///
	/// Use this when a plugin implements the same interface types as its binding
	/// but exports one or more interfaces or functions under different names.
	///
	/// The outer map is a lookup table from requested interface name to [`Remap`].
	/// Each [`Remap`] describes where that requested interface, and optionally
	/// requested items inside it, are found in this plugin's exports.
	///
	/// All remap tables use the same direction:
	///
	/// ```text
	/// requested name -> exported name
	/// ```
	///
	/// ```
	/// # use std::collections::HashMap ;
	/// # use wasm_link::sync::Plugin;
	/// # use wasm_link::{ PluginContext, ResourceTable, Component, Engine, Remap };
	/// # struct Ctx { resource_table: ResourceTable }
	/// # impl PluginContext for Ctx {
	/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
	/// # }
	/// # fn example( engine: &Engine ) -> Result<(), Box<dyn std::error::Error>> {
	/// let plugin = Plugin::new(
	/// 	Component::new( engine, "(component)" )?,
	/// 	Ctx { resource_table: ResourceTable::new() },
	/// ).remap_interfaces( HashMap::from([
	/// 	( "root".to_string(), Remap::found_as( "legacy-root" )),
	/// ]));
	/// # let _ = plugin ;
	/// # Ok(())
	/// # }
	/// ```
	pub fn remap_interfaces( mut self, interface_remaps: HashMap<String, Remap> ) -> Self {
		self.interface_remaps = interface_remaps ;
		self
	}

	/// Links this plugin with its socket bindings and instantiates it.
	///
	/// Takes ownership of the `linker` because socket bindings are added to it. If you need
	/// to reuse the same linker for multiple plugins, clone it before passing it in.
	///
	/// # Type Parameters
	/// - `PluginId`: Must implement `Into<Val>` so plugin IDs can be passed to WASM when
	/// 	dispatching to multi-plugin sockets (the ID identifies which plugin produced each result).
	///
	/// # Errors
	/// Returns an error if linking or instantiation fails.
	pub fn link<PluginId, Sockets>(
		self,
		engine: &Engine,
		mut linker: Linker<Ctx>,
		sockets: Sockets,
	) -> Result<sync::PluginInstance<Ctx>, wasmtime::Error>
	where
		PluginId: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<PluginId, Ctx>>,
	{
		sockets.into_iter()
			.map( Into::into )
			.try_for_each(| binding | binding.add_to_linker( &mut linker ))?;
		Self::instantiate( self, engine, &linker )
	}

	/// Asynchronously links this plugin with its socket bindings and instantiates it.
	///
	/// Use this variant when any socket may suspend or uses Component Model async types.
	/// Every plugin in an asynchronously linked graph should be created with
	/// [`instantiate_async`](Self::instantiate_async) or `link_async`.
	/// Calls to the returned instance are submitted to `executor`, allowing a
	/// thread pool to drive many independent plugin stores without reserving a
	/// worker for each plugin.
	///
	/// # Example
	///
	/// ```
	/// # use wasm_link::concurrent::{ BindingAny, Plugin };
	/// # use wasm_link::{ Component, Engine, Linker, PluginContext, ResourceTable };
	/// # struct Context { table: ResourceTable }
	/// # impl PluginContext for Context { fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table } }
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> { futures::executor::block_on( async {
	/// let engine = Engine::default();
	/// let linker = Linker::new( &engine );
	/// let executor = futures::executor::ThreadPool::new()?;
	/// let instance = Plugin::new(
	/// 	Component::new( &engine, "(component)" )?,
	/// 	Context { table: ResourceTable::new() },
	/// ).link(
	/// 	&engine,
	/// 	linker,
	/// 	Vec::<BindingAny<String, Context>>::new(),
	/// 	executor,
	/// ).await?;
	/// # let _ = instance;
	/// # Ok(()) }) }
	/// ```
	///
	/// # Errors
	/// Returns an error if linking or instantiation fails.
	pub async fn link_async<PluginId, Sockets, Executor>(
		self,
		engine: &Engine,
		mut linker: Linker<Ctx>,
		sockets: Sockets,
		executor: Executor,
	) -> Result<concurrent::PluginInstance<Ctx>, wasmtime::Error>
	where
		PluginId: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<PluginId, Ctx, concurrent::PluginInstance<Ctx>>>,
		Executor: Spawn + Send + Sync + 'static,
	{
		sockets.into_iter()
			.map( Into::into )
			.try_for_each(| binding | binding.add_to_linker_async( &mut linker ))?;
		Self::instantiate_async( self, engine, &linker, executor ).await
	}

	/// A convenience alias for [`Plugin::link`] with 0 sockets
	///
	/// # Errors
	/// Returns an error if instantiation fails.
	pub fn instantiate(
		self,
		engine: &Engine,
		linker: &Linker<Ctx>
	) -> Result<sync::PluginInstance<Ctx>, wasmtime::Error> {
		let mut store = Store::new( engine, self.context );
		if let Some( fuel ) = self.initial_fuel { store.set_fuel( fuel )?; }
		if let Some( limiter ) = self.memory_limiter { store.limiter( limiter ); }
		let instance = linker.instantiate( &mut store, &self.component )?;
		Ok( sync::PluginInstance::new_sync(
			store,
			instance,
			self.interface_remaps,
			self.fuel_limiter,
			self.epoch_limiter,
		))
	}

	/// Asynchronously instantiates this plugin.
	///
	/// This variant is required for WIT async functions, asynchronous host functions,
	/// and plugins that will be used in a graph created with [`link_async`](Self::link_async).
	/// Calls to the returned instance are submitted to `executor`. This keeps each
	/// plugin's [`Store`](wasmtime::Store) isolated while allowing a thread pool to
	/// drive many plugin stores without dedicating an idle thread to each one.
	/// Wasmtime concurrency support, which is enabled by default, must not be disabled
	/// on the `engine` used for asynchronous instances.
	///
	/// # Example
	///
	/// ```
	/// # use wasm_link::concurrent::Plugin;
	/// # use wasm_link::{ Component, Engine, Linker, PluginContext, ResourceTable };
	/// # struct Context { table: ResourceTable }
	/// # impl PluginContext for Context { fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table } }
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> { futures::executor::block_on( async {
	/// let engine = Engine::default();
	/// let linker = Linker::new( &engine );
	/// let executor = futures::executor::ThreadPool::new()?;
	/// let instance = Plugin::new(
	/// 	Component::new( &engine, "(component)" )?,
	/// 	Context { table: ResourceTable::new() },
	/// ).instantiate( &engine, &linker, executor ).await?;
	/// # let _ = instance;
	/// # Ok(()) }) }
	/// ```
	///
	/// # Errors
	/// Returns an error if instantiation fails.
	pub async fn instantiate_async<Executor>(
		self,
		engine: &Engine,
		linker: &Linker<Ctx>,
		executor: Executor,
	) -> Result<concurrent::PluginInstance<Ctx>, wasmtime::Error>
	where
		Executor: Spawn + Send + Sync + 'static,
	{
		let mut store = Store::new( engine, self.context );
		if let Some( fuel ) = self.initial_fuel { store.set_fuel( fuel )?; }
		if let Some( limiter ) = self.memory_limiter { store.limiter( limiter ); }
		let instance = linker.instantiate_async( &mut store, &self.component ).await?;
		Ok( concurrent::PluginInstance::new(
			store,
			instance,
			self.interface_remaps,
			self.fuel_limiter,
			self.epoch_limiter,
			executor,
		))
	}

}

/// A plugin declaration whose runtime is selected by its instance type.
#[must_use = "call .instantiate() or .link() to create a plugin instance"]
pub struct RuntimePlugin<Ctx: 'static, Instance>(
	Plugin<Ctx>,
	std::marker::PhantomData<fn() -> Instance>,
);

impl<Ctx, Instance> RuntimePlugin<Ctx, Instance>
where
	Ctx: PluginContext + 'static,
{
	/// Creates a plugin declaration.
	pub fn new( component: Component, context: Ctx ) -> Self {
		Self( Plugin::new( component, context ), std::marker::PhantomData )
	}

	/// Sets the fuel available during instantiation.
	pub fn with_initial_fuel( mut self, fuel: u64 ) -> Self {
		self.0 = self.0.with_initial_fuel( fuel );
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
}

impl<Ctx> RuntimePlugin<Ctx, sync::PluginInstance<Ctx>>
where
	Ctx: PluginContext + 'static,
{
	/// Sets the per-call fuel limiter.
	pub fn with_fuel_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &crate::sync::Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_fuel_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &crate::sync::Function::from_metadata( function ))
		});
		self
	}

	/// Sets the per-call epoch deadline limiter.
	pub fn with_epoch_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &crate::sync::Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_epoch_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &crate::sync::Function::from_metadata( function ))
		});
		self
	}

	/// Links socket bindings and instantiates the plugin.
	///
	/// # Errors
	///
	/// Returns an error when linking, validation, or instantiation fails.
	pub fn link<Id, Sockets>(
		self,
		engine: &Engine,
		linker: Linker<Ctx>,
		sockets: Sockets,
	) -> Result<sync::PluginInstance<Ctx>, wasmtime::Error>
	where
		Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<Id, Ctx>>,
	{
		self.0.ensure_synchronous( engine )?;
		self.0.link( engine, linker, sockets )
	}

	/// Instantiates a plugin with no socket bindings.
	///
	/// # Errors
	///
	/// Returns an error when synchronous instantiation fails.
	pub fn instantiate(
		self,
		engine: &Engine,
		linker: &Linker<Ctx>,
	) -> Result<sync::PluginInstance<Ctx>, wasmtime::Error> {
		self.0.ensure_synchronous( engine )?;
		self.0.instantiate( engine, linker )
	}
}

impl<Ctx> RuntimePlugin<Ctx, concurrent::PluginInstance<Ctx>>
where
	Ctx: PluginContext + 'static,
{
	/// Sets the per-call fuel limiter.
	pub fn with_fuel_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &crate::concurrent::Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_fuel_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &crate::concurrent::Function::from_metadata( function ))
		});
		self
	}

	/// Sets the per-call epoch deadline limiter.
	pub fn with_epoch_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &crate::concurrent::Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_epoch_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &crate::concurrent::Function::from_metadata( function ))
		});
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
	) -> Result<concurrent::PluginInstance<Ctx>, wasmtime::Error>
	where
		Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<Id, Ctx, concurrent::PluginInstance<Ctx>>>,
		Executor: Spawn + Send + Sync + 'static,
	{
		self.0.link_async( engine, linker, sockets, executor ).await
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
	) -> Result<concurrent::PluginInstance<Ctx>, wasmtime::Error>
	where
		Executor: Spawn + Send + Sync + 'static,
	{
		self.0.instantiate_async( engine, linker, executor ).await
	}
}

impl<Ctx: std::fmt::Debug + 'static, Instance> std::fmt::Debug for RuntimePlugin<Ctx, Instance> {
	fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		self.0.fmt( formatter )
	}
}

fn component_contains_async( engine: &Engine, component: &ComponentType ) -> bool {
	component.imports( engine ).any(|( _, item )| item_contains_async( engine, item.ty ))
		|| component.exports( engine ).any(|( _, item )| item_contains_async( engine, item.ty ))
}

fn instance_contains_async( engine: &Engine, instance: &ComponentInstance ) -> bool {
	instance.exports( engine ).any(|( _, item )| item_contains_async( engine, item.ty ))
}

fn item_contains_async( engine: &Engine, item: ComponentItem ) -> bool {
	// Wasmtime currently rejects component-valued imports and exports, but recurse here so
	// synchronous validation remains correct if support is added upstream.
	match item { ComponentItem::Component( component ) => component_contains_async( engine, &component ),
		ComponentItem::ComponentFunc( function ) => function.async_(),
		ComponentItem::ComponentInstance( instance ) => instance_contains_async( engine, &instance ),
		ComponentItem::CoreFunc( _ )
		| ComponentItem::Module( _ )
		| ComponentItem::Type( _ )
		| ComponentItem::Resource( _ ) => false,
	}
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		f.debug_struct( "Plugin" )
			.field( "component", &"<Component>" )
			.field( "context", &self.context )
			.field( "interface_remaps", &self.interface_remaps )
			.field( "initial_fuel", &self.initial_fuel )
			.field( "fuel_limiter", &self.fuel_limiter.as_ref().map(| _ | "<closure>" ))
			.field( "epoch_limiter", &self.epoch_limiter.as_ref().map(| _ | "<closure>" ))
			.field( "memory_limiter", &self.memory_limiter.as_ref().map(| _ | "<closure>" ))
			.finish_non_exhaustive()
	}
}
