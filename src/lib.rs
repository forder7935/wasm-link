//! A framework for building modular applications from WebAssembly plugins.
//!
//! Plugins are small, single-purpose WASM components that connect through abstract
//! interfaces. Each plugin declares a **plug** (the interface it implements) and
//! zero or more **sockets** (interfaces it depends on). The framework links these
//! into a dependency tree and handles cross-plugin dispatch.
//!
//! # Core Concepts
//!
//! - **Interface**: A contract declaring what an implementer exports and what a
//!   consumer may import. Defined via [`InterfaceData`].
//!
//! - **Plug**: A plugin's declaration that it implements an interface (exports its functions).
//!
//! - **Socket**: A plugin's declaration that it depends on an interface (expects to call
//!   into implementations provided by other plugins).
//!
//! - **Cardinality**: Each interface specifies how many plugins may implement it via
//!   [`InterfaceCardinality`]. This affects how dispatch results are returned.
//!
//! - **Root Socket**: The entry point interface that the host application calls into.
//!   Other interfaces are internal - only accessible to plugins, not the host.
//!
//! # Usage
//! ```
//! use wasm_link::{
//!     InterfaceData, InterfaceCardinality, FunctionData, ReturnKind,
//!     PluginData, PluginTree, Socket, Engine, Component, Linker, Val,
//! };
//!
//! // Declare your fixture sources
//! #[derive( Clone )]
//! struct Func { name: String, return_kind: ReturnKind }
//! impl FunctionData for Func {
//!     fn name( &self ) -> &str { self.name.as_str() }
//!     fn return_kind( &self ) -> ReturnKind { self.return_kind.clone() }
//!     // Determine whether a function is a resource method
//!     // a constructor is not considered to be a method
//!     fn is_method( &self ) -> bool { false }
//! }
//!
//! struct Interface { id: &'static str, funcs: Vec<Func> }
//! impl InterfaceData for Interface {
//!     type Id = &'static str ;
//!     type Error = std::convert::Infallible ;
//!     type Function = Func ;
//!     type FunctionIter<'a> = std::slice::Iter<'a, Func> ;
//!     type ResourceIter<'a> = std::iter::Empty<&'a String> ;
//!     fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
//!     fn cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> { Ok( &InterfaceCardinality::ExactlyOne ) }
//!     fn package_name( &self ) -> Result<&str, Self::Error> { Ok( "my:package/example" ) }
//!     fn functions( &self ) -> Result<Self::FunctionIter<'_>, Self::Error> { Ok( self.funcs.iter()) }
//!     fn resources( &self ) -> Result<Self::ResourceIter<'_>, Self::Error> { Ok( std::iter::empty()) }
//! }
//!
//! struct Plugin { id: &'static str, plug: &'static str }
//! impl PluginData for Plugin {
//!     type Id = &'static str ;
//!     type InterfaceId = &'static str ;
//!     type Error = std::convert::Infallible ;
//!     type SocketIter<'a> = std::iter::Empty<&'a Self::InterfaceId> ;
//!     fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
//!     fn plug( &self ) -> Result<&Self::InterfaceId, Self::Error> { Ok( &self.plug ) }
//!     fn sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> { Ok( std::iter::empty()) }
//!     fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> {
//!         /* inialise your component here */
//! #       Ok( Component::new( engine, r#"(component
//! #           (core module $m (func (export "f") (result i32) i32.const 42))
//! #           (core instance $i (instantiate $m))
//! #           (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
//! #           (instance $inst (export "get-value" (func $f)))
//! #           (export "my:package/example" (instance $inst))
//! #       )"# ).unwrap())
//!     }
//! }
//!
//! // Now construct some plugins and related data
//! let root_interface_id = "root" ;
//! let plugins = [ Plugin { id: "foo", plug: root_interface_id }];
//! let interfaces = [ Interface { id: root_interface_id, funcs: vec![
//!     Func { name: "get-value".to_string(), return_kind: ReturnKind::MayContainResources }
//! ]}];
//!
//! // First you need to tell wasm_link about your plugins, interfaces and where you want
//! // the execution to begin. wasm_link will try it's best to load in all the plugins,
//! // upon encountering an error, it will try to salvage as much of the remaining data
//! // as possible returning a list of failures alongside the `PluginTree`.
//! let ( tree, build_errors ) = PluginTree::new( root_interface_id, interfaces, plugins );
//! assert!( build_errors.is_empty() );
//!
//! // Once you've got your `PluginTree` constructed, you can link the plugins together
//! // Since some plugins may fail to load, it is only at this point that the cardinality
//! // requirements are satisfied by the plugins that managed to get loaded, otherwise it
//! // tries to salvage as much of the tree as can be loaded returning a list of failures
//! // alongside the loaded `PluginTreeHead` - the root node of the `PluginTree`.
//! let engine = Engine::default();
//! let linker = Linker::new( &engine );
//! let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
//! assert!( load_errors.is_empty() );
//!
//! // Now you can dispatch any function on the root interface.
//! // This will dispatch the function for all plugins plugged in to the root socket returning
//! // a Result for each in the shape determined by the interface cardinality.
//! let result = tree_head.dispatch( "my:package/example", "get-value", true, &[] );
//! match result {
//!     Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
//!     Socket::ExactlyOne( Err( e )) => panic!( "dispatch error: {e}" ),
//!     _ => panic!( "unexpected cardinality" ),
//! }
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
pub use wasmtime::component::{ Component, Linker, Val };

pub use interface::{ InterfaceData, InterfaceCardinality, FunctionData, ReturnKind };
pub use plugin::{ PluginData };
pub use loading::LoadError ;
pub use plugin_tree::{ PluginTree, PluginTreeError };
pub use plugin_tree_head::PluginTreeHead ;
pub use socket::Socket ;
pub use plugin_instance::DispatchError ;
pub use utils::{ PartialSuccess, PartialResult };
