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
//!
//! 1. Implement [`InterfaceData`] and [`PluginData`] for your metadata source
//!    (filesystem, database, embedded, etc.)
//!
//! 2. Build a [`PluginTree`] from your interfaces and plugins
//!
//! 3. Call [`PluginTree::load`] to compile WASM and link dependencies
//!
//! 4. Use [`PluginTreeHead::dispatch`] to invoke functions on the root interface
//!
//! # Example
//!
//! ```ignore
//! // Build the unloaded dependency graph
//! let (tree, build_errors) = PluginTree::new(
//!     root_interface_id,
//!     interfaces,
//!     plugins,
//! );
//!
//! // Compile and link all plugins
//! let linker = Linker::new(&engine);
//! let tree_head = tree.load(&engine, &linker)?;
//!
//! // Dispatch a function call to all root plugins
//! let results = tree_head.dispatch(
//!     "my:package/root",
//!     "hello",
//!     true,
//!     &[Val::String("world".into())],
//! );
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

pub use interface::{ InterfaceId, InterfaceData, InterfaceCardinality, FunctionData, ReturnKind };
pub use plugin::{ PluginId, PluginData };
pub use loading::{ LoadError, DispatchError };
pub use plugin_tree::{ PluginTree, PluginTreeError };
pub use plugin_tree_head::PluginTreeHead ;
pub use socket::Socket ;
pub use utils::{ PartialSuccess, PartialResult };
