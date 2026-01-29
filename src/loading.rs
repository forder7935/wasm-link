//! Plugin loading, compilation, and linking.
//!
//! This module handles the transformation from an unloaded [`PluginTree`] into a
//! running [`PluginTreeHead`]. The loading process:
//!
//! 1. Traverses the dependency graph starting from the root interface
//! 2. Compiles each plugin's WASM binary into a wasmtime component
//! 3. Links plugin sockets to their implementations (re-exports plug functions into sockets)
//! 4. Validates cardinality constraints are satisfied
//!
//! [`PluginTree`]: crate::PluginTree
//! [`PluginTreeHead`]: crate::PluginTreeHead

mod load_plugin_tree ;
mod load_socket ;
mod load_plugin ;
mod linker ;
mod resource_wrapper ;

pub use load_plugin_tree::{ LoadError, DispatchError };
pub(crate) use load_plugin_tree::load_plugin_tree ;
use load_plugin_tree::LoadResult ;
use load_socket::{ SocketState, load_socket };
pub(crate) use linker::LoadedSocket ;
use linker::link_socket ;
use resource_wrapper::{ ResourceWrapper, ResourceCreationError, ResourceReceiveError };
