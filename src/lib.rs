//! A WebAssembly plugin runtime for building modular applications.
//!
//! Plugins are small, single-purpose WASM components that connect through abstract
//! bindings. Each plugin declares a **plug** (the binding it implements) and
//! zero or more **sockets** (bindings it depends on). `wasm_link` links these
//! into a directed acyclic graph (DAG) and handles cross-plugin dispatch.
//!
//! # Core Concepts
//!
//! - [`Binding`]: An abstract contract declaring what an implementer exports and what a
//!   consumer may import. Contains a package name, a set of interfaces, and plugged-in
//!   plugin instances.
//!
//! - [`Interface`]: A single WIT interface with functions and resources. Note that
//!   interfaces don't have a name field; their names are provided as keys of a `HashMap`
//!   when constructing a [`Binding`]. This prevents duplicate interface names.
//!
//! - [`Plugin`]: A struct containing a wasm component and the runtime context made available
//!   to host exports; their ids are provided as keys of a `HashMap` when constructing a
//!   [`Binding`]. This prevents duplicate ids.
//!
//! - [`PluginInstance`]( plugin_instance::PluginInstance ): An instantiated plugin with its
//!   store and instance, ready for dispatch.
//!
//! - **Plug**: A plugin's declaration that it implements a [`Binding`].
//!
//! - **Socket**: A plugin's declaration that it depends on a [`Binding`]. Sockets are
//!   represented by the [`Socket`] enum, whose variant encodes **cardinality** - how many
//!   plugins may implement the dependency:
//!   - `ExactlyOne( Id, T )` - exactly one plugin, guaranteed present
//!   - `AtMostOne( Option<( Id, T )> )` - zero or one plugin
//!   - `AtLeastOne( HashMap<Id, T> )` - one or more plugins
//!   - `Any( HashMap<Id, T> )` - zero or more plugins
//!
//!   While cardinality is conceptually a property of bindings, it is represented by
//!   variants of the [`Socket`] enum due to how the DAG is constructed.
//!
//! # Example
//!
//! ```
//! use std::collections::{ HashMap, HashSet };
//! use wasm_link::{
//!     Binding, Interface, Function, ReturnKind,
//!     Plugin, PluginContext, Socket,
//!     Engine, Component, Linker, ResourceTable, Val,
//! };
//!
//! // First, declare a plugin context, the data stored inside wasmtime `Store<T>`.
//! // It must contain a resource table to implement `PluginContext` which is needed
//! // for ownership tracking of wasm component model resources.
//! struct Context { resource_table: ResourceTable }
//!
//! impl PluginContext for Context {
//!     fn resource_table( &mut self ) -> &mut ResourceTable {
//!         &mut self.resource_table
//!     }
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
//! // Plugin IDs are specified in the Socket variant to prevent duplicate ids.
//! let leaf = Plugin::new(
//!     Component::new( &engine, "(component)" )?,
//!     Context { resource_table: ResourceTable::new() },
//! ).instantiate( &engine, &linker )?;
//!
//! // Bindings expose a plugin's exports to other plugins.
//! // Socket variant sets cardinality: ExactlyOne, AtMostOne (0-1), AtLeastOne (1+), Any (0+).
//! let leaf_binding = Binding::new(
//!     "empty:package",
//!     HashMap::new(),
//!     Socket::ExactlyOne( "leaf".to_string(), leaf ),
//! );
//!
//! // `link()` wires up dependencies - this plugin can now import from leaf_binding.
//! let root = Plugin::new(
//!     Component::new( &engine, r#"(component
//!         (core module $m (func (export "f") (result i32) i32.const 42))
//!         (core instance $i (instantiate $m))
//!         (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
//!         (instance $inst (export "get-value" (func $f)))
//!         (export "my:package/example" (instance $inst))
//!     )"# )?,
//!     Context { resource_table: ResourceTable::new() },
//! ).link( &engine, linker, vec![ leaf_binding ])?;
//!
//! // Interface tells `wasm_link` which functions exist and how to handle returns.
//! let root_binding = Binding::new(
//!     "my:package",
//!     HashMap::from([( "example".to_string(), Interface::new(
//!         HashMap::from([
//!             ( "get-value".into(), Function::new( ReturnKind::MayContainResources, false ))
//!         ]),
//!         HashSet::new(),
//!     ))]),
//!     Socket::ExactlyOne( "root".to_string(), root ),
//! );
//!
//! // Now you can call into the plugin graph from the host.
//! let result = root_binding.dispatch( "example", "get-value", &[ /* args */ ] )?;
//! match result {
//!     Socket::ExactlyOne( _, Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
//!     Socket::ExactlyOne( _, Err( err )) => panic!( "dispatch error: {}", err ),
//!     _ => unreachable!(),
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
//! # use wasm_link::{ Binding, Plugin, PluginContext, Socket, Engine, Component, Linker, ResourceTable };
//! # struct Context { resource_table: ResourceTable }
//! # impl PluginContext for Context {
//! #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # impl Context {
//! #   pub fn new() -> Self { Self { resource_table: ResourceTable::new() } }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine = Engine::default();
//! # let linker = Linker::new( &engine );
//! let plugin_d = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//!     .instantiate( &engine, &linker )?;
//! let binding_d = Binding::new( "d:pkg", HashMap::new(), Socket::ExactlyOne( "D".to_string(), plugin_d ));
//!
//! // Both B and C import from D. Clone the binding handle so both can reference it.
//! let plugin_b = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//!     .link( &engine, linker.clone(), vec![ binding_d.clone() ])?;
//! let plugin_c = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//!     .link( &engine, linker.clone(), vec![ binding_d ])?;
//!
//! let binding_b = Binding::new( "b:pkg", HashMap::new(), Socket::ExactlyOne( "B".to_string(), plugin_b ));
//! let binding_c = Binding::new( "c:pkg", HashMap::new(), Socket::ExactlyOne( "C".to_string(), plugin_c ));
//!
//! let plugin_a = Plugin::new( Component::new( &engine, "(component)" )?, Context::new())
//!     .link( &engine, linker, vec![ binding_b, binding_c ])?;
//! # let _ = plugin_a ;
//! # Ok(())
//! # }
//! ```
//!
//! # Multiple Plugins Per Binding
//!
//! A single binding can have multiple plugin implementations. Use `Socket::AtLeastOne`
//! when at least one implementation is required, or `Socket::Any` when zero is acceptable.
//! When you dispatch to such a binding, you get results from all plugins.
//!
//! ```
//! # use std::collections::{ HashMap, HashSet };
//! # use wasm_link::{
//! #     Binding, Interface, Function, ReturnKind, Plugin, PluginContext,
//! #     Socket, Engine, Component, Linker, ResourceTable, Val,
//! # };
//! # struct Context { resource_table: ResourceTable }
//! # impl Context {
//! #   pub fn new() -> Self { Self { resource_table: ResourceTable::new() } }
//! # }
//! # impl PluginContext for Context {
//! #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let engine = Engine::default();
//! # let linker = Linker::new( &engine );
//! // Plugin IDs are specified through the HashMap keys for Socket::Any.
//! let plugin1 = Plugin::new( Component::new( &engine, r#"(component
//!     (core module $m (func (export "f") (result i32) i32.const 1))
//!     (core instance $i (instantiate $m))
//!     (func $f (result u32) (canon lift (core func $i "f")))
//!     (instance $inst (export "get-value" (func $f)))
//!     (export "pkg:interface/root" (instance $inst))
//! )"# )?, Context::new()).instantiate( &engine, &linker )?;
//!
//! let plugin2 = Plugin::new( Component::new( &engine, r#"(component
//!     (core module $m (func (export "f") (result i32) i32.const 2))
//!     (core instance $i (instantiate $m))
//!     (func $f (result u32) (canon lift (core func $i "f")))
//!     (instance $inst (export "get-value" (func $f)))
//!     (export "pkg:interface/root" (instance $inst))
//! )"# )?, Context::new()).instantiate( &engine, &linker )?;
//!
//! let binding = Binding::new(
//!     "pkg:interface",
//!     HashMap::from([( "root".to_string(), Interface::new(
//!         HashMap::from([( "get-value".into(), Function::new( ReturnKind::MayContainResources, false ))]),
//!         HashSet::new(),
//!     ))]),
//!     Socket::Any( HashMap::from([
//!         ( "p1".to_string(), plugin1 ),
//!         ( "p2".to_string(), plugin2 ),
//!     ])),
//! );
//!
//! // Dispatch calls all plugins; the result Socket variant matches what you passed in.
//! let results = binding.dispatch( "root", "get-value", &[] )?;
//! match results {
//!     Socket::Any( map ) => {
//!         assert_eq!( map.len(), 2 );
//!         assert!( matches!( map.get( "p1" ), Some( Ok( Val::U32( 1 )))));
//!         assert!( matches!( map.get( "p2" ), Some( Ok( Val::U32( 2 )))));
//!     },
//!     _ => unreachable!(),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Resource Limits (Fuel & Epochs)
//!
//! Plugins may run untrusted code, so `wasm_link` supports Wasmtime's fuel and epoch
//! mechanisms to prevent runaway execution. Both must be enabled in your
//! [`Engine`] configuration:
//!
//! - **Fuel** counts WebAssembly instructions. When fuel runs out, execution traps.
//!   Enable with [`Config::consume_fuel`]( wasmtime::Config::consume_fuel ).
//!
//! - **Epochs** count external timer ticks. When the deadline is reached, execution
//!   traps. Enable with [`Config::epoch_interruption`]( wasmtime::Config::epoch_interruption ).
//!
//! ## Setting Limits
//!
//! Limits can be set at three levels, listed from lowest to highest precedence:
//!
//! 1. **Binding default** - applies to all functions in the binding
//! 2. **Function-specific** - overrides the binding default for one function
//! 3. **Plugin override** - highest precedence, set per-plugin per-function
//!
//! Plugins can also set a **multiplier** that scales the base value (from function
//! or binding).
//!
//! ```
//! # use std::collections::{ HashMap, HashSet };
//! # use wasm_link::{ Binding, Interface, Function, ReturnKind, Plugin, PluginContext, Socket, Component, Linker, ResourceTable };
//! # use wasmtime::{ Config, Engine };
//! # struct Context { resource_table: ResourceTable }
//! # impl Context { fn new() -> Self { Self { resource_table: ResourceTable::new() }}}
//! # impl PluginContext for Context {
//! #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Enable fuel consumption in the engine
//! let mut config = Config::new();
//! config.consume_fuel( true );
//! let engine = Engine::new( &config )?;
//! let linker = Linker::new( &engine );
//!
//! # let component = Component::new( &engine, "(component)" )?;
//! // Slower plugin gets double the fuel
//! let slow = Plugin::new( component, Context::new() )
//!     .with_fuel_multiplier( 2.0 )
//!     .instantiate( &engine, &linker )?;
//!
//! // Binding with a default fuel limit; "expensive-fn" gets more
//! let binding = Binding::<String, _>::build(
//!     "my:pkg",
//!     HashMap::from([( "api".into(), Interface::new(
//!         HashMap::from([
//!             ( "cheap-fn".into(), Function::new( ReturnKind::Void, false )),
//!             ( "expensive-fn".into(), Function::new( ReturnKind::Void, false )
//!                 .with_fuel( 100_000 )), // Override for this function
//!         ]),
//!         HashSet::new(),
//!     ))]),
//!     Socket::ExactlyOne( "plugin".into(), slow ),
//! )
//!     .with_default_fuel( 10_000 ) // Binding-wide default
//!     .build();
//! # Ok(())
//! # }
//! ```
//!
//! ## Important Notes
//!
//! **Engine configuration is required.** Fuel and epoch limits only work when enabled
//! in the [`Engine`] configuration. For more information, look into [`wasmtime`] docs.
//!
//! **Fuel and epoch are independent.** A function can have both a fuel limit and an
//! epoch deadline. They are resolved and applied separately; whichever is exhausted
//! first causes a trap.
//!
//! **Invalid override keys are silently ignored.** If a plugin specifies a fuel or
//! epoch override for an `(interface, function)` pair that doesn't exist, the override
//! is never used. No error is raised.
//!
//! **Engine enabled but no limits set.** If you enable fuel/epochs in the [`Engine`]
//! but don't set any limits in `wasm_link`, the behavior mimics the wasmtime behaviour.
//! - *Fuel*: A fresh [`Store`]( wasmtime::Store ) starts with 0 fuel, so the first
//!   instruction immediately traps. This is likely not what you want.
//! - *Epochs*: No deadline is set, so execution runs indefinitely regardless of epoch
//!   ticks.

mod binding ;
mod interface ;
mod plugin ;
mod plugin_instance ;
mod socket ;
mod linker ;
mod resource_wrapper ;

pub use wasmtime::Engine ;
pub use wasmtime::component::{ Component, Linker, ResourceTable, Val };
pub use nonempty_collections::{ NEMap, nem };

pub use binding::Binding ;
pub use interface::{ Interface, Function, ReturnKind };
pub use plugin::{ PluginContext, Plugin };
pub use plugin_instance::{ PluginInstance, DispatchError };
pub use socket::Socket ;
