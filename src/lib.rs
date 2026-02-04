//! A WebAssembly plugin runtime for building modular applications.
//!
//! Plugins are small, single-purpose WASM components that connect through abstract
//! bindings. Each plugin declares a **plug** (the binding it implements) and
//! zero or more **sockets** (bindings it depends on). `wasm_link` links these
//! into a dependency tree and handles cross-plugin dispatch.
//!
//! # Core Concepts
//!
//! - [`Binding`]: A contract declaring what an implementer exports and what a
//!   consumer may import.
//!
//! - **Plug**: A plugin's declaration that it implements a [`Binding`].
//!
//! - **Socket**: A plugin's declaration that it depends on a [`Binding`].
//!
//! - [`Cardinality`]: Each binding specifies how many plugins may implement it.
//!   This affects how dispatch results are returned.
//!
//! - **Root Binding**: The entry point [`Binding`] that the host application calls into.
//!   Other bindings are internal - only accessible to plugins, not the host.
//!
//! # Example
//! ```
//! use wasm_link::{
//!     Binding, Interface, Function, Cardinality, ReturnKind,
//!     Plugin, PluginContext, PluginTree, Socket,
//!     Engine, Component, Linker, ResourceTable, Val,
//! };
//!
//! // Define a context that implements PluginContext
//! struct Context {
//!     resource_table: ResourceTable,
//! }
//!
//! impl PluginContext for Context {
//!     fn resource_table( &mut self ) -> &mut ResourceTable {
//!         &mut self.resource_table
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = Engine::default();
//!
//! // Start by defining your root binding that will be used to interface with the plugin tree
//! const ROOT_BINDING: &str = "root" ;
//! const EXAMPLE_INTERFACE: &str = "example" ;
//! const GET_VALUE: &str = "get-value" ;
//!
//! let binding = Binding::new(
//!     ROOT_BINDING,
//!     Cardinality::ExactlyOne,
//!     "my:package",
//!     vec![ Interface::new(
//!         EXAMPLE_INTERFACE,
//!         vec![ Function::new( GET_VALUE, ReturnKind::MayContainResources, false ) ],
//!         Vec::<String>::with_capacity( 0 ),
//!     )],
//! );
//!
//! // Now create a plugin that implements this binding
//! let plugin = Plugin::new(
//!     "foo",
//!     ROOT_BINDING,
//!     Vec::with_capacity( 0 ),
//!     Component::new( &engine, r#"(component
//!         (core module $m (func (export "f") (result i32) i32.const 42))
//!         (core instance $i (instantiate $m))
//!         (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
//!         (instance $inst (export "get-value" (func $f)))
//!         (export "my:package/example" (instance $inst))
//!     )"# )?,
//!     Context { resource_table: ResourceTable::new() },
//! );
//!
//! // First you need to tell `wasm_link` about your plugins, bindings and where you want
//! // the execution to begin. `wasm_link` will try it's best to load in all the plugins,
//! // upon encountering an error, it will try to salvage as much of the remaining data
//! // as possible returning a list of failures alongside the `PluginTree`.
//! let ( tree, init_errors ) = PluginTree::new( ROOT_BINDING, vec![ binding ], vec![ plugin ] );
//! assert!( init_errors.is_empty() );
//!
//! // Once you've got your `PluginTree` constructed, you can link the plugins together
//! // Since some plugins may fail to load, it is only at this point that the cardinality
//! // requirements are validated depending on the plugins that managed to get loaded,
//! // otherwise it tries to salvage as much of the tree as can be loaded returning a list
//! // of failures alongside the loaded `PluginTreeHead` - the root node of the `PluginTree`.
//! let linker = Linker::new( &engine );
//! let ( tree_head, load_errors ) = tree.load( &engine, &linker ).map_err(|( e, _ )| e )?;
//! assert!( load_errors.is_empty() );
//!
//! // Dispatch a function call to plugins implementing the root binding
//! let result = tree_head.dispatch( EXAMPLE_INTERFACE, GET_VALUE, true, &[] );
//! match result {
//!     Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
//!     Socket::ExactlyOne( Err( err )) => panic!( "dispatch error: {}", err ),
//!     _ => panic!( "unexpected cardinality" ),
//! }
//! # Ok(())
//! # }
//! ```

mod interface ;
mod plugin ;
mod loading ;
mod plugin_tree ;
mod plugin_tree_head ;
mod socket ;
mod plugin_instance ;
mod utils ;

pub use wasmtime::Engine ;
pub use wasmtime::component::{ Component, Linker, ResourceTable, Val };

pub use interface::{ Binding, Interface, Function, Cardinality, ReturnKind };
pub use plugin::{ PluginContext, Plugin };
pub use loading::LoadError ;
pub use plugin_tree::{ PluginTree, PluginTreeError };
pub use plugin_tree_head::PluginTreeHead ;
pub use socket::Socket ;
pub use plugin_instance::DispatchError ;
pub use utils::{ PartialSuccess, PartialResult };
