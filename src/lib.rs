//! WebAssembly plugin runtime with separate [`sync`] and [`concurrent`] APIs.
//!
//! Use [`sync`] for entirely synchronous component graphs and [`concurrent`] when
//! any component uses WIT-async functions or calls may suspend. Runtime-state types
//! from the two modules are intentionally incompatible.

mod binding ;
mod runtime_binding ;
pub mod concurrent ;
mod interface ;
mod plugin ;
mod plugin_instance ;
mod remap ;
pub mod cardinality ;
#[cfg(test)] mod cardinality_tests ;
#[cfg(test)] mod interface_tests ;
mod linker ;
mod resource_wrapper ;
pub mod sync ;

#[doc( no_inline )]
pub use wasmtime::Engine ;
#[doc( no_inline )]
pub use wasmtime::component::{ Component, Linker, ResourceTable, Val };
#[doc( no_inline )]
pub use nonempty_collections::{ NEMap, nem };

pub use interface::{ FunctionKind, ReturnKind };
pub use plugin::PluginContext ;
pub use plugin_instance::DispatchError ;
pub use remap::{ ItemResolutionTable, Remap };
pub use resource_wrapper::{ ResourceCreationError, ResourceReceiveError };
