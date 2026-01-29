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
/// Obtained from [`PluginTree::load`]. This is the host application's entry point
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
    /// * `has_return` - Whether the function returns a value (use [`FunctionData::has_return`])
    /// * `data` - Arguments to pass to the function as wasmtime [`Val`]s
    ///
    /// # Example
    /// ```ignore
    /// let results = tree_head.dispatch(
    ///     "my:package/greeter",
    ///     "greet",
    ///     true,  // has return value
    ///     &[Val::String("world".into())],
    /// );
    ///
    /// match results {
    ///     Socket::ExactlyOne(Ok(val)) => println!("Got: {:?}", val),
    ///     Socket::ExactlyOne(Err(e)) => eprintln!("Error: {}", e),
    ///     // ... handle other cardinalities
    /// }
    /// ```
    ///
    /// [`FunctionData::has_return`]: crate::FunctionData::has_return
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
