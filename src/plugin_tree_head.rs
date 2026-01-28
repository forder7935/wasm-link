use std::sync::{ Arc, RwLock };
use wasmtime::component::Val ;

use crate::interface::InterfaceData ;
use crate::plugin::PluginData ;
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use crate::loading::DispatchError ;



/// The root node of a loaded plugin tree.
///
/// Obtained from [`PluginTree::load`]. Provides access to dispatch functions
/// on the root socket's plugins.
pub struct PluginTreeHead<I: InterfaceData, P: PluginData + 'static> {
    /// Retained for future hot-loading support (adding/removing plugins at runtime).
    pub(crate) _interface: Arc<I>,
    pub(crate) socket: Arc<Socket<RwLock<PluginInstance<P>>>>,
}

impl<I: InterfaceData, P: PluginData> PluginTreeHead<I, P> {
    /// Invokes a function on all plugins in the root socket.
    ///
    /// Returns a [`Socket`] containing each plugin's result or error. The socket
    /// variant mirrors the root interface's cardinality.
    ///
    /// # Arguments
    /// * `interface_path` - WIT interface path (e.g., `"my:package/root"`)
    /// * `function` - Function name to call
    /// * `has_return` - Whether the function returns a value
    /// * `data` - Arguments to pass to the function
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
