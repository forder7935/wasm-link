//! Binding specification and metadata types.
//!
//! A [`Binding`] defines an abstract contract specifying what plugins must implement
//! (via plugs) or what they could depend on (via sockets). It bundles one or more WIT
//! [`Interface`]s under a single identifier.

use std::sync::{ Arc, Mutex };
use std::collections::HashMap ;
use wasmtime::component::{ Linker, Val };

use crate::{ Interface, PluginContext };
use crate::cardinality::{ Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne };
use crate::plugin_instance::PluginInstance ;



type PluginSockets<PluginId, Ctx, Plugins> =
    <Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>> ;

type DispatchResults<PluginId, Ctx, Plugins> =
    <PluginSockets<PluginId, Ctx, Plugins> as Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>>::Rebind<
        Result<wasmtime::component::Val, crate::DispatchError>
    >;

type DispatchVals<PluginId, Ctx, Plugins> =
    <PluginSockets<PluginId, Ctx, Plugins> as Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>>::Rebind<
        wasmtime::component::Val
    >;

struct BindingData<PluginId, Ctx, Plugins>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>>,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync,
{
    package_name: String,
    interfaces: HashMap<String, Interface>,
    plugins: PluginSockets<PluginId, Ctx, Plugins>,
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
/// # use std::collections::{ HashMap, HashSet };
/// # use wasm_link::{ Binding, Interface, Function, FunctionKind, ReturnKind, Plugin, ExactlyOne, Engine, Component, Linker, ResourceTable };
/// # struct Ctx { resource_table: ResourceTable }
/// # impl wasm_link::PluginContext for Ctx {
/// #     fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
/// # }
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let engine = Engine::default();
/// # let linker = Linker::new( &engine );
/// # let plugin = Plugin::new( Component::new( &engine, "(component)" )?, Ctx { resource_table: ResourceTable::new() }).instantiate( &engine, &linker )?;
/// let binding: Binding<String, Ctx> = Binding::new(
///     "my:package",
///     HashMap::from([
///         ( "api".to_string(), Interface::new(
///             HashMap::from([( "get-value".into(), Function::new(
///                 FunctionKind::Freestanding,
///                 ReturnKind::MayContainResources,
///             ))]),
///             HashSet::from([ "my-resource".to_string() ]),
///         )),
///     ]),
///     ExactlyOne( "my-plugin".to_string(), plugin ),
/// );
///
/// // Clone for shared dependencies - both refer to the same binding
/// let binding_clone = binding.clone();
/// # Ok(())
/// # }
/// ```
///
/// # Type Parameters
/// - `PluginId`: Unique identifier type for plugins (e.g., `String`, `UUID`)
pub struct Binding<PluginId, Ctx, Plugins = ExactlyOne<PluginId, PluginInstance<Ctx>>>(Arc<BindingData<PluginId, Ctx, Plugins>>)
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>> + 'static,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync;

impl<PluginId, Ctx, Plugins> Clone for Binding<PluginId, Ctx, Plugins>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>> + 'static,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync,
{
    fn clone( &self ) -> Self {
        Self( Arc::clone( &self.0 ))
    }
}

impl<PluginId, Ctx, Plugins> std::fmt::Debug for Binding<PluginId, Ctx, Plugins>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
    Ctx: PluginContext + std::fmt::Debug + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>> + 'static,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync,
    PluginSockets<PluginId, Ctx, Plugins>: std::fmt::Debug,
{
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        f.debug_struct( "Binding" )
            .field( "package_name", &self.0.package_name )
            .field( "interfaces", &self.0.interfaces )
            .field( "plugins", &self.0.plugins )
            .finish()
    }
}

impl<PluginId, Ctx, Plugins> Binding<PluginId, Ctx, Plugins>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>> + 'static,
    PluginSockets<PluginId, Ctx, Plugins>: Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync,
{

    /// Creates a new binding specification.
    pub fn new(
        package_name: impl Into<String>,
        interfaces: HashMap<String, Interface>,
        plugins: Plugins
    ) -> Self {
        Self( Arc::new( BindingData {
            package_name: package_name.into(),
            interfaces,
            plugins: plugins.map_mut( Mutex::new ),
        }))
    }

    pub(crate) fn add_to_linker( binding: &Binding<PluginId, Ctx, Plugins>, linker: &mut Linker<Ctx> ) -> Result<(), wasmtime::Error>
    where
        PluginId: Into<Val>,
        DispatchVals<PluginId, Ctx, Plugins>: Into<Val>,
    {
        binding.0.interfaces.iter().try_for_each(|( name, interface )| {
            let interface_ident = format!( "{}/{}", binding.0.package_name, name );
            interface.add_to_linker( linker, &interface_ident, binding )
        })
    }

    pub(crate) fn plugins( &self ) -> &PluginSockets<PluginId, Ctx, Plugins> {
        &self.0.plugins
    }

    /// Dispatches a function call to all plugins implementing this binding.
    ///
    /// This is used for external dispatch (calling into the plugin graph from outside).
    /// The result is wrapped in a type matching the binding's cardinality.
    ///
    /// # Arguments
    /// * `interface_name` - The interface name within this binding (e.g., "example")
    /// * `function_name` - The function name within the interface (e.g., "get-value")
    /// * `args` - Arguments to pass to the function
    ///
    /// # Returns
    /// A cardinality wrapper containing `Result<Val, DispatchError>` for each plugin.
    /// For [`ReturnKind::Void`]( crate::ReturnKind::Void ), the value is an empty tuple
    /// (`Val::Tuple( vec![] )`) placeholder.
    ///
    /// # Errors
    /// Returns an error if the interface or function is not found in this binding.
    pub fn dispatch(
        &self,
        interface_name: &str,
        function_name: &str,
        args: &[wasmtime::component::Val],
    ) -> Result<DispatchResults<PluginId, Ctx, Plugins>, crate::DispatchError> {

        let interface = self.0.interfaces.get( interface_name )
            .ok_or_else(|| crate::DispatchError::InvalidInterfacePath( format!( "{}/{}", self.0.package_name, interface_name )))?;

        let function = interface.function( function_name )
            .ok_or_else(|| crate::DispatchError::InvalidFunction( function_name.to_string() ))?;

        let interface_path = format!( "{}/{}", self.0.package_name, interface_name );

        Ok( self.0.plugins.map(| _, plugin | plugin
            .lock().map_err(|_| crate::DispatchError::LockRejected )
            .and_then(| mut lock | lock.dispatch(
                &interface_path,
                function_name,
                function,
                args,
            ))
        ))

    }

}

/// Type-erased binding wrapper for heterogeneous socket lists.
///
/// Use when a plugin's sockets include bindings with different cardinalities.
#[derive( Debug, Clone )]
pub enum BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    /// Exactly one plugin implementation.
    ExactlyOne( Binding<PluginId, Ctx, ExactlyOne<PluginId, PluginInstance<Ctx>>> ),
    /// Zero or one plugin implementation.
    AtMostOne( Binding<PluginId, Ctx, AtMostOne<PluginId, PluginInstance<Ctx>>> ),
    /// One or more plugin implementations.
    AtLeastOne( Binding<PluginId, Ctx, AtLeastOne<PluginId, PluginInstance<Ctx>>> ),
    /// Zero or more plugin implementations.
    Any( Binding<PluginId, Ctx, Any<PluginId, PluginInstance<Ctx>>> ),
}

impl<PluginId, Ctx> BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
    Ctx: PluginContext + 'static,
{
    pub(crate) fn add_to_linker( &self, linker: &mut Linker<Ctx> ) -> Result<(), wasmtime::Error> {
        match self {
            Self::ExactlyOne( binding ) => Binding::add_to_linker( binding, linker ),
            Self::AtMostOne( binding ) => Binding::add_to_linker( binding, linker ),
            Self::AtLeastOne( binding ) => Binding::add_to_linker( binding, linker ),
            Self::Any( binding ) => Binding::add_to_linker( binding, linker ),
        }
    }
}

impl<PluginId, Ctx> From<Binding<PluginId, Ctx, ExactlyOne<PluginId, PluginInstance<Ctx>>>> for BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn from( binding: Binding<PluginId, Ctx, ExactlyOne<PluginId, PluginInstance<Ctx>>> ) -> Self {
        Self::ExactlyOne( binding )
    }
}

impl<PluginId, Ctx> From<Binding<PluginId, Ctx, AtMostOne<PluginId, PluginInstance<Ctx>>>> for BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn from( binding: Binding<PluginId, Ctx, AtMostOne<PluginId, PluginInstance<Ctx>>> ) -> Self {
        Self::AtMostOne( binding )
    }
}

impl<PluginId, Ctx> From<Binding<PluginId, Ctx, AtLeastOne<PluginId, PluginInstance<Ctx>>>> for BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn from( binding: Binding<PluginId, Ctx, AtLeastOne<PluginId, PluginInstance<Ctx>>> ) -> Self {
        Self::AtLeastOne( binding )
    }
}

impl<PluginId, Ctx> From<Binding<PluginId, Ctx, Any<PluginId, PluginInstance<Ctx>>>> for BindingAny<PluginId, Ctx>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    fn from( binding: Binding<PluginId, Ctx, Any<PluginId, PluginInstance<Ctx>>> ) -> Self {
        Self::Any( binding )
    }
}

impl<PluginId, Ctx, Plugins> Binding<PluginId, Ctx, Plugins>
where
    PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
    Plugins: Cardinality<PluginId, PluginInstance<Ctx>>,
    PluginSockets<PluginId, Ctx, Plugins>: Send + Sync,
    BindingAny<PluginId, Ctx>: From<Binding<PluginId, Ctx, Plugins>>,
{
    /// Converts this binding into a type-erased [`BindingAny`] for heterogeneous socket lists.
    pub fn into_any( self ) -> BindingAny<PluginId, Ctx> {
        self.into()
    }
}
