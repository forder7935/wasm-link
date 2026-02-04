use std::sync::Arc ;
use std::collections::HashMap ;
use thiserror::Error ;
use wasmtime::Engine;
use wasmtime::component::Linker ;

use crate::{ Binding, Cardinality };
use crate::{ Plugin, PluginContext } ;
use crate::utils::PartialResult ;
use super::{ load_socket, SocketState, LoadedSocket };



/// Errors that can occur while loading and linking plugins.
///
/// Loading attempts to proceed gracefully, collecting errors and loading as many
/// plugins as possible while adhering to cardinality constraints. These errors
/// are returned via [`PartialResult`] from [`PluginTree::load`].
///
/// [`PartialResult`]: crate::PartialResult
/// [`PluginTree::load`]: crate::PluginTree::load
#[derive( Error )]
pub enum LoadError<BindingId> {

    /// A plugin references a socket (dependency) that isn't present in
    /// the given [`PluginTree`]( crate::PluginTree )
    #[error( "Invalid socket: {0}" )]
    InvalidSocket( BindingId ),

    /// A dependency cycle was detected. Cycles are forbidden in the plugin graph.
    #[error( "Loop detected loading: '{0}'" )]
    LoopDetected( BindingId ),

    /// The number of plugins implementing a binding violates its cardinality requirements.
    #[error( "Failed to meet cardinality requirements: {0}, found {1}" )]
    FailedCardinalityRequirements( Cardinality, usize ),

    /// Wasmtime failed to instantiate the component.
    #[error( "Failed to load component: {0}" )]
    FailedToLoadComponent( wasmtime::Error ),

    /// Failed to link the root interface into the plugin.
    #[error( "Failed to link root interface: {0}" )]
    FailedToLinkInterface( wasmtime::Error ),

    /// Failed to link a specific function during socket wiring.
    #[error( "Failed to link function '{0}': {1}" )]
    FailedToLink( String, wasmtime::Error ),

    /// Internal marker for errors that have already been reported.
    #[error( "Handled failure" )]
    AlreadyHandled,

}

impl<BindingId: std::fmt::Debug> std::fmt::Debug for LoadError<BindingId> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        match self {
            Self::InvalidSocket( id ) => f.debug_tuple( "InvalidSocket" ).field( id ).finish(),
            Self::LoopDetected( id ) => f.debug_tuple( "LoopDetected" ).field( id ).finish(),
            Self::FailedCardinalityRequirements( c, n ) => f.debug_tuple( "FailedCardinalityRequirements" ).field( c ).field( n ).finish(),
            Self::FailedToLoadComponent( e ) => f.debug_tuple( "FailedToLoadComponent" ).field( e ).finish(),
            Self::FailedToLinkInterface( e ) => f.debug_tuple( "FailedToLinkInterface" ).field( e ).finish(),
            Self::FailedToLink( name, e ) => f.debug_tuple( "FailedToLink" ).field( name ).field( e ).finish(),
            Self::AlreadyHandled => f.debug_struct( "AlreadyHandled" ).finish(),
        }
    }
}

/// Result of a load operation that may have partial failures.
/// The `errors` field contains handled load failures
/// Convenience abstraction semantically equivalent to:
/// `( SocketMap, LoadResult<T, LoadError, LoadError> )`
pub(super) struct LoadResult<T, BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq,
    PluginId: Clone,
    Ctx: PluginContext + 'static,
{
    pub socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    pub result: Result<T, LoadError<BindingId>>,
    pub errors: Vec<LoadError<BindingId>>,
}

#[inline]
#[allow( clippy::type_complexity )]
pub(crate) fn load_plugin_tree<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, ( Binding<BindingId>, Vec<Plugin<PluginId, BindingId, Ctx>> )>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    root: BindingId,
) -> PartialResult<( Arc<Binding<BindingId>>, Arc<LoadedSocket<PluginId, Ctx>> ), LoadError<BindingId>>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    let socket_map = socket_map.into_iter()
        .map(|( socket_id, ( binding, plugins ))| ( socket_id, SocketState::Unprocessed( binding, plugins )))
        .collect();

    match load_socket( socket_map, engine, default_linker, root ) {
        LoadResult { socket_map: _, result: Ok(( binding, socket )), errors } => Ok((( binding, socket ), errors )),
        LoadResult { socket_map: _, result: Err( err ), errors } => Err(( err, errors ))
    }

}
