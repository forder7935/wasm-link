//! Entry point for dispatching function calls to loaded plugins.
//!
//! The [`PluginTreeHead`] represents a fully loaded and linked plugin tree.
//! It provides access to the root socket - the entry point interface that
//! the host application calls into.

use std::sync::{ Arc, RwLock };
use wasmtime::component::Val ;

use crate::interface::InterfaceData ;
use crate::plugin::PluginData ;
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use crate::loading::DispatchError ;



/// The root node of a loaded plugin tree.
///
/// Obtained from [`PluginTree::load`](crate::PluginTree::load). This is the host application's entry point
/// for calling into the plugin system. The root socket represents the interface
/// that the host has access to - all other interfaces are internal and can only
/// be called by other plugins.
///
/// The host acts as a pseudo-plugin: it doesn't need to be implemented in WASM
/// and has access to system capabilities that plugins don't.
pub struct PluginTreeHead<I: InterfaceData, P: PluginData + 'static> {
    /// Retained for future hot-loading support (adding/removing plugins at runtime).
    pub(crate) _interface: Arc<I>,
    pub(crate) socket: Arc<Socket<RwLock<PluginInstance<P>>>>,
}

impl<I: InterfaceData, P: PluginData> PluginTreeHead<I, P> {
    /// Invokes a function on all plugins in the root socket.
    ///
    /// Dispatches the function call to every plugin implementing the root interface.
    /// The return type is a [`Socket`] whose variant matches the interface's cardinality:
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
    /// use wasm_compose::{
    ///     InterfaceId, InterfaceData, InterfaceCardinality, FunctionData, ReturnKind,
    ///     PluginId, PluginData, PluginTree, Engine, Component, Linker, Socket, Val,
    /// };
    ///
    /// #[derive( Clone )]
    /// struct Func { name: String, return_kind: ReturnKind }
    /// impl FunctionData for Func {
    ///     /* .. */
    /// #   fn name( &self ) -> &str { self.name.as_str() }
    /// #   fn return_kind( &self ) -> ReturnKind { self.return_kind.clone() }
    /// #   fn is_method( &self ) -> bool { false }
    /// }
    ///
    /// struct Interface { id: InterfaceId, funcs: Vec<Func> }
    /// impl InterfaceData for Interface {
    ///     /* ... */
    /// #   type Error = std::convert::Infallible ;
    /// #   type Function = Func ;
    /// #   type FunctionIter<'a> = std::slice::Iter<'a, Func> ;
    /// #   type ResourceIter<'a> = std::iter::Empty<&'a String> ;
    /// #   fn id( &self ) -> Result<InterfaceId, Self::Error> { Ok( self.id ) }
    /// #   fn cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> {
    /// #       Ok( &InterfaceCardinality::ExactlyOne )
    /// #   }
    /// #   fn package_name( &self ) -> Result<&str, Self::Error> { Ok( "my:package/example" ) }
    /// #   fn functions( &self ) -> Result<Self::FunctionIter<'_>, Self::Error> {
    /// #       Ok( self.funcs.iter())
    /// #   }
    /// #   fn resources( &self ) -> Result<Self::ResourceIter<'_>, Self::Error> {
    /// #       Ok( std::iter::empty())
    /// #   }
    /// }
    ///
    /// struct Plugin { id: PluginId, plug: InterfaceId }
    /// impl PluginData for Plugin {
    ///     /* ... */
    /// #   type Error = std::convert::Infallible ;
    /// #   type SocketIter<'a> = std::iter::Empty<&'a InterfaceId> ;
    /// #   fn id( &self ) -> Result<&PluginId, Self::Error> { Ok( &self.id ) }
    /// #   fn plug( &self ) -> Result<&InterfaceId, Self::Error> { Ok( &self.plug ) }
    /// #   fn sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> {
    /// #       Ok( std::iter::empty())
    /// #   }
    /// #   fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> {
    /// #       Ok( Component::new( engine, r#"(component
    /// #           (core module $m (func (export "f") (result i32) i32.const 42))
    /// #           (core instance $i (instantiate $m))
    /// #           (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
    /// #           (instance $inst (export "get-value" (func $f)))
    /// #           (export "my:package/example" (instance $inst))
    /// #       )"# ).unwrap())
    /// #   }
    /// }
    ///
    /// let root_interface_id = InterfaceId::new( 0 );
    /// let plugins = [ Plugin { id: PluginId::new( 1 ), plug: root_interface_id }];
    /// let interfaces = [ Interface { id: root_interface_id, funcs: vec![
    ///     Func { name: "get-value".to_string(), return_kind: ReturnKind::MayContainResources }
    /// ]}];
    ///
    /// let ( tree, build_errors ) = PluginTree::new( root_interface_id, interfaces, plugins );
    /// assert!( build_errors.is_empty() );
    ///
    /// let engine = Engine::default();
    /// let linker = Linker::new( &engine );
    /// let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
    /// assert!( load_errors.is_empty() );
    ///
    /// // Dispatch returns a Socket matching the interface's cardinality
    /// let result = tree_head.dispatch( "my:package/example", "get-value", true, &[] );
    ///
    /// match result {
    ///     Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
    ///     Socket::ExactlyOne( Err( e )) => panic!( "dispatch error: {e}" ),
    ///     _ => panic!( "unexpected cardinality" ),
    /// }
    /// ```
    pub fn dispatch<IE>(
        &self,
        interface_path: &str,
        function: &str,
        has_return: bool,
        data: &[Val],
    ) -> Socket<Result<Val, DispatchError<IE>>>
    where
        IE: std::error::Error,
        I: InterfaceData<Error = IE>,
    {
        self.socket.dispatch_function( interface_path, function, has_return, data )
    }
}
