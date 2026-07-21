//! A WebAssembly plugin runtime for building modular applications.
//!
//! The runtime has separate [`sync`] and [`concurrent`] APIs so entirely synchronous
//! component graphs do not pay for asynchronous execution. Use [`concurrent`] when
//! any component uses WIT-async functions or a call may suspend.

mod binding ;
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

#[doc(hidden)]
pub use binding::{ Binding as BindingBase, BindingAny as BindingAnyBase };
#[doc(hidden)]
pub use plugin::RuntimePlugin as PluginBase;
