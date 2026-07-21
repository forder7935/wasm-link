//! Synchronous plugin runtime.
//!
//! This runtime cannot execute WebAssembly functions declared with the WIT `async`
//! effect. Encountering one while linking or instantiating a component returns a
//! runtime error; use [`crate::concurrent`] for async-capable plugin graphs.
//!
//! Runtime-state types are distinct from [`crate::concurrent`] and cannot be mixed
//! with them in one plugin tree.
//!
//! A WebAssembly plugin runtime for building modular applications.
//!
//! Plugins are small, single-purpose WASM components that connect through abstract
//! bindings. Each plugin declares a **plug** (the binding it implements) and
//! zero or more **sockets** (bindings it depends on). `wasm_link` links these
//! into a directed acyclic graph (DAG) and handles cross-plugin dispatch.
//!
//! # Core Concepts
//!
//! - [`crate::sync::Binding`]: An abstract contract declaring what an implementer exports and what a
//! 	consumer may import. Contains a package name, a set of interfaces, and plugged-in
//! 	plugin instances.
//!
//! - [`crate::sync::Interface`]: A single WIT interface with functions and resources. Note that
//! 	interfaces don't have a name field; their names are provided as keys of a `HashMap`
//! 	when constructing a [`crate::sync::Binding`]. This prevents duplicate interface names.
//!
//! - [`crate::sync::Plugin`]: A struct containing a wasm component and the runtime context made available
//! 	to host exports; their ids are provided as keys of a `HashMap` when constructing a
//! 	[`crate::sync::Binding`]. This prevents duplicate ids.
//!
//! - [`crate::sync::PluginInstance`] and [`crate::concurrent::PluginInstance`]: Instantiated plugins ready
//! 	for synchronous or concurrent dispatch.
//!
//! - **Plug**: A plugin's declaration that it implements a [`crate::sync::Binding`].
//!
//! - **Socket**: A plugin's declaration that it depends on a [`crate::sync::Binding`]. Cardinality is
//! 	expressed with wrapper types in [`crate::cardinality`], and socket responses are
//! 	represented in the importing plugin's ABI using the corresponding shape:
//! 	- [`crate::cardinality::ExactlyOne`]`( Id, T )` - exactly one plugin,
//!			represented as `tuple<PluginId, result<T>>`
//! 	- [`crate::cardinality::AtMostOne`]`( Option<( Id, T )> )` - zero or one plugin,
//!			represented as `option<tuple<PluginId, result<T>>>`
//! 	- [`crate::cardinality::AtLeastOne`]`( nonempty_collections::NEMap<Id, T> )` - one or more plugins,
//!			represented as `map<PluginId, result<T>>`
//! 	- [`crate::cardinality::Any`]`( HashMap<Id, T> )` - zero or more plugins,
//!			represented as `map<PluginId, result<T>>`
//!
//! # Re-exports
//!
//! `wasm_link` re-exports a small set of types from `wasmtime` for convenience
//! (`Engine`, `Component`, `Linker`, `ResourceTable`, `Val`). These types are
//! defined by wasmtime; see the [wasmtime docs](https://docs.rs/wasmtime/latest/wasmtime/)
//! for details.
//!
//! # Example
//!
//! ```
//! use std::collections::{ HashMap, HashSet };
//! use wasm_link::sync::{ Binding, Function, Interface, Plugin };
//! use wasm_link::{
//! 	FunctionKind, ReturnKind, PluginContext, Engine, Component, Linker, ResourceTable, Val,
//! };
//! use wasm_link::cardinality::ExactlyOne ;
//!
//! // First, declare a plugin context, the data stored inside wasmtime `Store<T>`.
//! // It must contain a resource table to implement `PluginContext` which is needed
//! // for ownership tracking of wasm component model resources.
//! struct Context { resource_table: ResourceTable }
//!
//! impl PluginContext for Context {
//! 	fn resource_table( &mut self ) -> &mut ResourceTable {
//! 		&mut self.resource_table
//! 	}
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // You create your own engine. This allows you to define your config but note that
//! // not all options are compatible. As a general rule of thumb, if an option changes
//! // the way you interact with wasm, it is likely not compatible since this is managed
//! // by `wasm_link` directly. If the option makes sense, it will likely be supported
//! // in the future through wasm_link options.
//! let engine = Engine::default();
//!
//! // Similarly you may create your own linker, which you can add any exports into.
//! // Such exports will be available to all the plugins. It is your responsibility to
//! // make sure these don't conflict with re-exports of plugins that some other plugin
//! // depends on as these too have to be added to the same linker.
//! let linker = Linker::new( &engine );
//!
//! // Build the DAG bottom-up: start with plugins that have no dependencies.
//! // Note that for plugins that don't require linking, you only need to pass in
//! // a reference to a linker. For plugins that have dependencies, the linker is mutated.
//! // Plugin IDs are specified in the cardinality wrapper to prevent duplicate ids.
//! let leaf = Plugin::new(
//! 	Component::new( &engine, "(component)" )?,
//! 	Context { resource_table: ResourceTable::new() },
//! ).instantiate( &engine, &linker )?;
//!
//! // Bindings expose a plugin's exports to other plugins.
//! // Wrapper sets cardinality: ExactlyOne, AtMostOne (0-1), AtLeastOne (1+), Any (0+).
//! let leaf_binding = Binding::new(
//! 	"empty:package",
//! 	HashMap::new(),
//! 	ExactlyOne( "leaf".to_string(), leaf ),
//! );
//!
//! // `link()` wires up dependencies - this plugin can now import from leaf_binding.
//! let root = Plugin::new(
//! 	Component::new( &engine, r#"(component
//! 		(core module $m (func (export "f") (result i32) i32.const 42))
//! 		(core instance $i (instantiate $m))
//! 		(func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
//! 		(instance $inst (export "get-value" (func $f)))
//! 		(export "my:package/example" (instance $inst))
//! 	)"# )?,
//! 	Context { resource_table: ResourceTable::new() },
//! ).link( &engine, linker, vec![ leaf_binding ])?;
//!
//! // Interface tells `wasm_link` which functions exist and how to handle returns.
//! let root_binding = Binding::new(
//! 	"my:package",
//! 	HashMap::from([( "example".to_string(), Interface::new(
//! 		HashMap::from([( "get-value".into(), Function::new(
//! 			FunctionKind::Freestanding, ReturnKind::MayContainResources,
//! 		))]),
//! 		HashSet::new(),
//! 	))]),
//! 	ExactlyOne( "root".to_string(), root ),
//! );
//!
//! // Now you can call into the plugin graph from the host.
//! let result = root_binding.dispatch( "example", "get-value", &[ /* args */ ] )?;
//! match result {
//! 	ExactlyOne( _id, Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
//! 	ExactlyOne( _id, Ok( _ )) => panic!( "unexpected response" ),
//! 	ExactlyOne( _id, Err( err )) => panic!( "dispatch error: {}", err ),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Shared Dependencies
//!
//! Sometimes multiple plugins need to depend on the same binding. Since `Binding`
//! is a handle type, cloning it creates another reference to the same underlying
//! binding rather than duplicating it.
//!
//! ```
//! # use std::collections::HashMap ;
//! # use wasm_link::sync::{ Binding, Plugin };
//! # use wasm_link::{ PluginContext, Engine, Component, Linker, ResourceTable };
//! # use wasm_link::cardinality::ExactlyOne ;
//! # struct Context { resource_table: ResourceTable }
//! # impl PluginContext for Context {
//! # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # impl Context {
//! # 	pub fn new() -> Self { Self { resource_table: ResourceTable::new() } }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine = Engine::default();
//! # let linker = Linker::new( &engine );
//! let plugin_d = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//! 	.instantiate( &engine, &linker )?;
//! let binding_d = Binding::new( "d:pkg", HashMap::new(), ExactlyOne( "D".to_string(), plugin_d ));
//!
//! // Both B and C import from D. Clone the binding handle so both can reference it.
//! let plugin_b = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//! 	.link( &engine, linker.clone(), vec![ binding_d.clone() ])?;
//! let plugin_c = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//! 	.link( &engine, linker.clone(), vec![ binding_d ])?;
//!
//! let binding_b = Binding::new( "b:pkg", HashMap::new(), ExactlyOne( "B".to_string(), plugin_b ));
//! let binding_c = Binding::new( "c:pkg", HashMap::new(), ExactlyOne( "C".to_string(), plugin_c ));
//!
//! let plugin_a = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//! 	.link( &engine, linker, vec![ binding_b, binding_c ])?;
//! # let _ = plugin_a ;
//! # Ok(())
//! # }
//! ```
//!
//! # Multiple Plugins Per Binding
//!
//! A single binding can have multiple plugin implementations. Use [`crate::cardinality::AtLeastOne`]
//! when at least one implementation is required, or [`crate::cardinality::Any`] when zero is acceptable.
//! When you dispatch to such a binding, you get results from all plugins.
//!
//! ```
//! # use std::collections::{ HashMap, HashSet };
//! # use wasm_link::sync::{ Binding, Function, Interface, Plugin };
//! # use wasm_link::{
//! # 	FunctionKind, ReturnKind, PluginContext, Engine, Component, Linker, ResourceTable, Val,
//! # };
//! # use wasm_link::cardinality::Any ;
//! # struct Context { resource_table: ResourceTable }
//! # impl Context {
//! # 	pub fn new() -> Self { Self { resource_table: ResourceTable::new() } }
//! # }
//! # impl PluginContext for Context {
//! # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine = Engine::default();
//! # let linker = Linker::new( &engine );
//! // Plugin IDs are specified through the HashMap keys for Any.
//! let plugin1 = Plugin::new( Component::new( &engine, r#"(component
//! 	(core module $m (func (export "f") (result i32) i32.const 1))
//! 	(core instance $i (instantiate $m))
//! 	(func $f (result u32) (canon lift (core func $i "f")))
//! 	(instance $inst (export "get-value" (func $f)))
//! 	(export "pkg:interface/root" (instance $inst))
//! )"# )?, Context::new()).instantiate( &engine, &linker )?;
//!
//! let plugin2 = Plugin::new( Component::new( &engine, r#"(component
//! 	(core module $m (func (export "f") (result i32) i32.const 2))
//! 	(core instance $i (instantiate $m))
//! 	(func $f (result u32) (canon lift (core func $i "f")))
//! 	(instance $inst (export "get-value" (func $f)))
//! 	(export "pkg:interface/root" (instance $inst))
//! )"# )?, Context::new()).instantiate( &engine, &linker )?;
//!
//! let binding = Binding::new(
//! 	"pkg:interface",
//! 	HashMap::from([( "root".to_string(), Interface::new(
//! 		HashMap::from([( "get-value".into(), Function::new(
//!				FunctionKind::Freestanding,
//!				ReturnKind::MayContainResources,
//!			))]),
//! 		HashSet::new(),
//! 	))]),
//! 	Any( HashMap::from([
//! 		( "p1".to_string(), plugin1 ),
//! 		( "p2".to_string(), plugin2 ),
//! 	])),
//! );
//!
//! // Dispatch calls all plugins; the result wrapper matches what you passed in.
//! let Any( map ) = binding.dispatch( "root", "get-value", &[] )?;
//! assert_eq!( map.len(), 2 );
//! assert!( matches!( map.get( "p1" ), Some( Ok( Val::U32( 1 )))));
//! assert!( matches!( map.get( "p2" ), Some( Ok( Val::U32( 2 )))));
//! # Ok(())
//! # }
//! ```
//!
//! # Resource Limits
//!
//! Plugins may run untrusted code. `wasm_link` exposes three mechanisms to control
//! resource usage:
//!
//! - **Fuel** counts WebAssembly instructions. When fuel runs out, execution traps.
//! 	Enable with [`Config::consume_fuel`]( wasmtime::Config::consume_fuel ).
//! 	Set initially via [`crate::sync::Plugin::with_initial_fuel`] and per-call via
//! 	[`crate::sync::Plugin::with_fuel_limiter`].
//!
//! - **Epoch deadline** counts external timer ticks. When the deadline is reached,
//! 	execution traps. Enable with [`Config::epoch_interruption`]( wasmtime::Config::epoch_interruption ).
//! 	Set per-call via [`crate::sync::Plugin::with_epoch_limiter`].
//!
//! - **Memory** limits linear memory and table growth via wasmtime's
//! 	[`ResourceLimiter`]( wasmtime::ResourceLimiter ). No engine configuration required.
//! 	Set once at instantiation via [`crate::sync::Plugin::with_memory_limiter`].
//!
//! ## Fuel and Epoch Limits
//!
//! Fuel and epoch limits are set per-plugin via closures that receive the store,
//! WIT interface path, function name, and function metadata. This gives you full
//! control over the limit per call.
//!
//! ```
//! # use std::collections::{ HashMap, HashSet };
//! # use wasm_link::sync::{ Binding, Function, Interface, Plugin };
//! # use wasm_link::{ FunctionKind, ReturnKind, PluginContext, Component, Linker, ResourceTable };
//! # use wasm_link::cardinality::ExactlyOne ;
//! # use wasmtime::{ Config, Engine };
//! # struct Context { resource_table: ResourceTable }
//! # impl Context { fn new() -> Self { Self { resource_table: ResourceTable::new() }}}
//! # impl PluginContext for Context {
//! # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Enable fuel consumption in the engine
//! let mut config = Config::new();
//! config.consume_fuel( true );
//! let engine = Engine::new( &config )?;
//! let linker = Linker::new( &engine );
//!
//! # let component = Component::new( &engine, "(component)" )?;
//! // Give this plugin a flat fuel budget per call
//! let plugin = Plugin::new( component, Context::new() )
//! 	.with_fuel_limiter(| _store, _interface, _function, _metadata | 100_000 )
//! 	.instantiate( &engine, &linker )?;
//!
//! let binding = Binding::<String, _>::new(
//! 	"my:pkg",
//! 	HashMap::from([( "api".into(), Interface::new(
//! 		HashMap::from([
//! 			( "cheap-fn".into(), Function::new( FunctionKind::Freestanding, ReturnKind::Void )),
//! 			( "expensive-fn".into(), Function::new( FunctionKind::Freestanding, ReturnKind::Void )),
//! 		]),
//! 		HashSet::new(),
//! 	))]),
//! 	ExactlyOne( "plugin".into(), plugin ),
//! );
//! # Ok(())
//! # }
//! ```
//! ## Important Notes
//!
//! **Engine configuration is required.** Fuel and epoch deadline limits only work when enabled
//! in the [`Engine`] configuration. Memory limits require no engine configuration.
//! For more information, see the [wasmtime docs](https://docs.rs/wasmtime/latest/wasmtime/).
//!
//! **Fuel and epoch deadlines are independent.** A function can have both a fuel limit and an
//! epoch deadline. They are applied separately; whichever is exhausted first causes
//! a trap.
//!
//! **Engine enabled but no limiter set.** If you enable fuel/epoch deadlines in the [`Engine`]
//! but don't set a limiter on the [`crate::sync::Plugin`], the behavior mimics the wasmtime default.
//! - *Fuel*: Without [`crate::sync::Plugin::with_initial_fuel`], a fresh [`Store`]
//! 	starts with 0 fuel. If component initialization executes fuel-metered Wasm, it
//! 	immediately traps. Without a per-call limiter, subsequent calls consume any initial
//! 	fuel remaining after initialization.
//! - *Epoch deadlines*: No deadline is set, so execution runs indefinitely regardless of epoch
//! 	ticks.
//!
//! ## Memory Limits
//!
//! Memory limits are implemented via wasmtime's [`ResourceLimiter`]( wasmtime::ResourceLimiter ),
//! which you implement and store inside your plugin context. The limiter is installed
//! once at instantiation and controls memory and table growth for the plugin's lifetime.
//! No engine configuration is required.
//!
//! ```
//! # use wasm_link::sync::Plugin;
//! # use wasm_link::{ PluginContext, ResourceTable, Component, Engine, Linker };
//! # use wasmtime::ResourceLimiter;
//! struct Ctx {
//! 	resource_table: ResourceTable,
//! 	limiter: MemoryLimiter,
//! }
//! impl PluginContext for Ctx {
//! 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! }
//!
//! struct MemoryLimiter { max_bytes: usize }
//! impl ResourceLimiter for MemoryLimiter {
//! 	fn memory_growing( &mut self, _current: usize, desired: usize, _max: Option<usize> ) -> wasmtime::Result<bool> {
//! 		Ok( desired <= self.max_bytes )
//! 	}
//! 	fn table_growing( &mut self, _current: usize, _desired: usize, _max: Option<usize> ) -> wasmtime::Result<bool> {
//! 		Ok( true )
//! 	}
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = Engine::default();
//! let linker = Linker::new( &engine );
//! # let component = Component::new( &engine, "(component)" )?;
//! let plugin = Plugin::new( component, Ctx {
//! 	resource_table: ResourceTable::new(),
//! 	limiter: MemoryLimiter { max_bytes: 10 * 1024 * 1024 }, // 10 MiB
//! }).with_memory_limiter(| ctx | &mut ctx.limiter )
//! 	.instantiate( &engine, &linker )?;
//! # let _ = plugin;
//! # Ok(())
//! # }
//! ```

use std::collections::{ HashMap, HashSet };
use wasmtime::component::{ Component, Linker, Val };
use wasmtime::{ Engine, Store };

use crate::cardinality::Cardinality;
use crate::interface::{ Function as FunctionMetadata, Interface as InterfaceMetadata };
use crate::plugin::Plugin as PluginCore;
use crate::plugin_instance::PluginInstanceSync;
use crate::runtime_binding::define_runtime_bindings;
use crate::{ DispatchError, FunctionKind, PluginContext, Remap, ReturnKind };

/// An instantiated plugin in the synchronous runtime.
pub use crate::plugin_instance::PluginInstanceSync as PluginInstance;

/// Metadata for a synchronous WIT function.
///
/// This type has no state capable of representing an asynchronous function.
#[derive( Debug, Clone )]
pub struct Function {
	kind: FunctionKind,
	return_kind: ReturnKind,
}

impl Function {
	/// Creates function metadata.
	pub fn new( kind: FunctionKind, return_kind: ReturnKind ) -> Self {
		Self { kind, return_kind }
	}

	/// Returns whether the function is freestanding or a resource method.
	pub fn kind( &self ) -> FunctionKind { self.kind }

	/// Returns how dispatch handles the function's return value.
	pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

	/// Returns whether the WIT function has the `async` effect.
	///
	/// Synchronous function metadata cannot represent that state, so this always
	/// returns `false`.
	pub fn is_async( &self ) -> bool { false }

	fn into_metadata( self ) -> FunctionMetadata {
		FunctionMetadata::new( self.kind, self.return_kind )
	}

	fn from_metadata( metadata: &FunctionMetadata ) -> Self {
		Self::new( metadata.kind(), metadata.return_kind() )
	}
}

/// A synchronous WIT interface declaration.
#[derive( Debug, Clone, Default )]
pub struct Interface {
	functions: HashMap<String, Function>,
	resources: HashSet<String>,
}

impl Interface {
	/// Creates an interface containing only synchronous functions.
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
	PluginInstanceSync,
	"A binding in the synchronous runtime.",
	"A synchronous binding with erased cardinality."
);

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Cardinality<
		Id,
		std::sync::Arc<futures::lock::Mutex<PluginInstanceSync<Ctx>>>,
	> + Send + Sync,
{
	/// Dispatches a function call.
	///
	/// # Errors
	///
	/// Returns an error if the interface or function is not declared by the binding.
	pub fn dispatch(
		&self,
		interface_name: &str,
		function_name: &str,
		args: &[Val],
	) -> Result<Results<Id, Ctx, Plugins>, DispatchError> {
		self.0.dispatch( interface_name, function_name, args )
	}
}

/// A component and context configured for the synchronous runtime.
#[must_use = "call .instantiate() or .link() to create a PluginInstance"]
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
	pub fn link<Id, Sockets>(
		self,
		engine: &Engine,
		linker: Linker<Ctx>,
		sockets: Sockets,
	) -> Result<PluginInstance<Ctx>, wasmtime::Error>
	where
		Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<Id, Ctx>>,
	{
		self.0.ensure_synchronous( engine )?;
		self.0.link(
			engine,
			linker,
			sockets.into_iter().map(| binding | binding.into().into_core()),
		)
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
	) -> Result<PluginInstance<Ctx>, wasmtime::Error> {
		self.0.ensure_synchronous( engine )?;
		self.0.instantiate( engine, linker )
	}
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
	fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		self.0.fmt( formatter )
	}
}
