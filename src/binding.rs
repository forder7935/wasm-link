//! Binding specification and metadata types.
//!
//! A [`Binding`] defines an abstract contract specifying what plugins must implement
//! (via plugs) or what they could depend on (via sockets). It bundles one or more WIT
//! [`Interface`]s under a single identifier.

use std::sync::{ Arc, Mutex };
use std::collections::HashMap ;
use wasmtime::component::{ Linker, Val };

use crate::{ Interface, PluginContext, Socket };
use crate::plugin_instance::PluginInstance ;



#[derive( Debug )]
pub struct BindingInner<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    package_name: String,
    interfaces: HashMap<String, Interface>,
    plugins: Socket<Mutex<PluginInstance<Ctx>>, PluginId>,
}

/// An abstract contract specifying what plugins must implement (via plugs) or what
/// they could depend on (via sockets). It bundles one or more WIT [`Interface`]s
/// under a single package name.
///
/// `Binding` is a handle to shared state. Cloning a `Binding` creates another handle
/// to the same underlying binding, enabling shared dependencies where multiple
/// plugins depend on the same binding.
///
/// ```
/// # use std::collections::HashMap;
/// # use wasm_link::{ Binding, Interface, Socket };
/// # struct Ctx { resource_table: wasm_link::ResourceTable }
/// # impl wasm_link::PluginContext for Ctx {
/// #     fn resource_table( &mut self ) -> &mut wasm_link::ResourceTable { &mut self.resource_table }
/// # }
/// # fn example() {
/// let binding: Binding<String, Ctx> = Binding::new(
///     "my:package",
///     HashMap::new(),
///     Socket::Any( HashMap::new() ),
/// );
///
/// // Clone for shared dependencies - both refer to the same binding
/// let binding_clone = binding.clone();
/// # }
/// ```
///
/// # Type Parameters
/// - `PluginId`: Unique identifier type for plugins (e.g., `String`, `UUID`)
pub struct Binding<PluginId, Ctx>(Arc<BindingInner<PluginId, Ctx>>)
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static;

impl<PluginId, Ctx> Clone for Binding<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn clone( &self ) -> Self {
        Self( Arc::clone( &self.0 ))
    }
}

impl<PluginId, Ctx> std::fmt::Debug for Binding<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
    Ctx: PluginContext + std::fmt::Debug + 'static,
{
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Binding" )
            .field( "package_name", &self.0.package_name )
            .field( "interfaces", &self.0.interfaces )
            .field( "plugins", &self.0.plugins )
            .finish()
    }
}

impl<PluginId, Ctx> Binding<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    /// Creates a new binding specification.
    #[inline]
    pub fn new(
        package_name: impl Into<String>,
        interfaces: HashMap<String, Interface>,
        plugins: Socket<PluginInstance<Ctx>, PluginId>
    ) -> Self {
        Self( Arc::new( BindingInner {
            package_name: package_name.into(),
            interfaces,
            plugins: plugins.map_mut( Mutex::new ),
        }))
    }

    pub(crate) fn add_to_linker( binding: &Binding<PluginId, Ctx>, linker: &mut Linker<Ctx> ) -> Result<(), wasmtime::Error>
    where PluginId: Into<Val>
    {
        binding.0.interfaces.iter().try_for_each(|( name, interface )| {
            let interface_ident = format!( "{}/{}", binding.0.package_name, name );
            interface.add_to_linker( linker, &interface_ident, binding )
        })
    }

    pub(crate) fn plugins( &self ) -> &Socket<Mutex<PluginInstance<Ctx>>, PluginId> {
        &self.0.plugins
    }

    /// Dispatches a function call to all plugins implementing this binding.
    ///
    /// This is used for external dispatch (calling into the plugin graph from outside).
    /// The result is wrapped in a [`Socket`] matching the binding's cardinality.
    ///
    /// # Arguments
    /// * `interface_name` - The interface name within this binding (e.g., "example")
    /// * `function_name` - The function name within the interface (e.g., "get-value")
    /// * `args` - Arguments to pass to the function
    ///
    /// # Returns
    /// A `Socket` containing `Result<Val, DispatchError>` for each plugin.
    ///
    /// # Errors
    /// Returns an error if the interface or function is not found in this binding.
    pub fn dispatch(
        &self,
        interface_name: &str,
        function_name: &str,
        args: &[wasmtime::component::Val],
    ) -> Result<Socket<Result<wasmtime::component::Val, crate::DispatchError>, PluginId>, crate::DispatchError> {

        let interface = self.0.interfaces.get( interface_name )
            .ok_or_else(|| crate::DispatchError::InvalidInterfacePath( format!( "{}/{}", self.0.package_name, interface_name )))?;

        let function = interface.function( function_name )
            .ok_or_else(|| crate::DispatchError::InvalidFunction( function_name.to_string() ))?;

        let interface_path = format!( "{}/{}", self.0.package_name, interface_name );
        let has_return = function.return_kind() != crate::interface::ReturnKind::Void ;

        Ok( self.0.plugins.dispatch_function( &interface_path, function_name, has_return, args ))

    }

}
