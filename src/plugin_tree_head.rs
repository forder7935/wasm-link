//! Entry point for dispatching function calls to loaded plugins.
//!
//! The [`PluginTreeHead`] represents a fully loaded and linked plugin tree.
//! It provides access to the root socket - the entry point binding that
//! the host application calls into.

use std::sync::{ Arc, Mutex };
use wasmtime::component::Val ;

use crate::interface::Binding ;
use crate::plugin::PluginContext ;
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use crate::DispatchError ;



/// The root node of a loaded plugin tree.
///
/// Obtained from [`PluginTree::load`]( crate::PluginTree::load ). This is the host application's entry point
/// for calling into the plugin system. The root socket represents the binding
/// that the host has access to - all other bindings are internal and can only
/// be called by other plugins.
///
/// The host acts as a pseudo-plugin: it doesn't need to be implemented in WASM
/// and has access to system capabilities that plugins don't.
///
/// # Type Parameters
/// - `BindingId`: Identifier type for bindings
/// - `PluginId`: Identifier type for plugins
/// - `Ctx`: User context type stored in wasmtime Store
pub struct PluginTreeHead<BindingId, PluginId, Ctx: 'static> {
    pub(crate) binding: Arc<Binding<BindingId>>,
    pub(crate) socket: Arc<Socket<Mutex<PluginInstance<PluginId, Ctx>>, PluginId>>,
}

impl<BindingId, PluginId, Ctx> PluginTreeHead<BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    /// Invokes a function on all plugins in the root socket.
    ///
    /// Dispatches the function call to every plugin implementing the root binding.
    /// The return type is a [`Socket`] whose variant matches the binding's cardinality:
    ///
    /// - `ExactlyOne` cardinality → `Socket::ExactlyOne(result)`
    /// - `AtMostOne` cardinality → `Socket::AtMostOne(Option<result>)`
    /// - `AtLeastOne` / `Any` cardinality → `Socket::AtLeastOne/Any(HashMap<PluginId, result>)`
    ///
    /// # Arguments
    /// * `interface_path` - Full WIT interface path (e.g., `"my:package/interface-name"`)
    /// * `function` - Function name to call as declared in the interface
    /// * `has_return` - Whether you expect to receive a return value
    /// * `data` - Arguments to pass to the function as wasmtime [`Val`]s
    ///
    /// # Example
    ///
    /// ```
    /// use wasm_link::{
    ///     Binding, Interface, Cardinality, Function, ReturnKind,
    ///     Plugin, PluginContext, PluginTree, Socket,
    ///     Engine, Component, Linker, ResourceTable, Val,
    /// };
    ///
    /// struct Context { resource_table: ResourceTable }
    /// impl PluginContext for Context {
    ///     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
    /// }
    ///
    /// let engine = Engine::default();
    ///
    /// const ROOT_BINDING: &str = "root" ;
    /// const EXAMPLE_INTERFACE: &str = "example" ;
    /// const GET_VALUE: &str = "get-value" ;
    ///
    /// let binding = Binding::new(
    ///     ROOT_BINDING,
    ///     Cardinality::ExactlyOne,
    ///     "my:package",
    ///     vec![ Interface::new(
    ///         EXAMPLE_INTERFACE,
    ///         vec![ Function::new( GET_VALUE, ReturnKind::MayContainResources, false ) ],
    ///         Vec::<String>::new(),
    ///     )],
    /// );
    ///
    /// let plugin = Plugin::new(
    ///     "foo",
    ///     ROOT_BINDING,
    ///     Vec::with_capacity( 0 ),
    ///     Component::new( &engine, r#"(component
    ///         (core module $m (func (export "f") (result i32) i32.const 42))
    ///         (core instance $i (instantiate $m))
    ///         (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
    ///         (instance $inst (export "get-value" (func $f)))
    ///         (export "my:package/example" (instance $inst))
    ///     )"# ).unwrap(),
    ///     Context { resource_table: ResourceTable::new() },
    /// );
    ///
    /// let ( tree, build_errors ) = PluginTree::new( ROOT_BINDING, vec![ binding ], vec![ plugin ] );
    /// assert!( build_errors.is_empty() );
    ///
    /// let linker = Linker::new( &engine );
    /// let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
    /// assert!( load_errors.is_empty() );
    ///
    /// // Dispatch returns a Socket matching the binding's cardinality
    /// let result = tree_head.dispatch( EXAMPLE_INTERFACE, GET_VALUE, true, &[] );
    ///
    /// match result {
    ///     Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
    ///     Socket::ExactlyOne( Err( e )) => panic!( "dispatch error: {e}" ),
    ///     _ => panic!( "unexpected cardinality" ),
    /// }
    /// ```
    pub fn dispatch(
        &self,
        interface: &str,
        function: &str,
        has_return: bool,
        data: &[Val],
    ) -> Socket<Result<Val, DispatchError>, PluginId> {
        self.socket.dispatch_function( &format!( "{}/{}", self.binding.package_name(), interface ), function, has_return, data )
    }
}
