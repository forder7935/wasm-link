use std::sync::Arc ;
use std::collections::HashMap ;
use thiserror::Error ;
use wasmtime::Engine;
use wasmtime::component::{ Linker, Val };

use crate::interface::{ InterfaceId, InterfaceData, InterfaceCardinality };
use crate::plugin::PluginData ;
use crate::utils::PartialResult ;
use super::{ load_socket, SocketState, LoadedSocket };
use super::{ ResourceCreationError, ResourceReceiveError };



/// Errors that can occur while loading and linking plugins.
///
/// Loading attempts to proceed gracefully, collecting errors and loading as many
/// plugins as possible while adhering to cardinality constraints. These errors
/// are returned via [`PartialResult`] from [`PluginTree::load`].
///
/// [`PartialResult`]: crate::PartialResult
/// [`PluginTree::load`]: crate::PluginTree::load
#[derive( Error )]
pub enum LoadError<I: InterfaceData, P: PluginData> {

    /// A plugin references a socket (dependency) that doesn't exist in the interface set.
    #[error( "Invalid socket: {0}" )]
    InvalidSocket( InterfaceId ),

    /// A dependency cycle was detected. Cycles are forbidden in the plugin graph.
    #[error( "Loop detected loading: '{0}'" )]
    LoopDetected( InterfaceId ),

    /// The number of plugins implementing an interface violates its cardinality.
    /// For example, `ExactlyOne` with 0 or 2+ plugins, or `AtLeastOne` with 0 plugins.
    #[error( "Failed to meet cardinality requirements: {0}, found {1}" )]
    FailedCardinalityRequirements( InterfaceCardinality, usize ),

    /// Failed to read interface metadata (e.g., couldn't parse WIT or access data source).
    #[error( "Corrupted interface manifest: {0}" )]
    CorruptedInterfaceManifest( I::Error ),

    /// Failed to read plugin metadata (e.g., couldn't determine sockets or ID).
    #[error( "Corrupted plugin manifest: {0}" )]
    CorruptedPluginManifest( P::Error ),

    /// Wasmtime failed to compile the WASM component (invalid binary or unsupported features).
    #[error( "Failed to load component: {0}" )]
    FailedToLoadComponent( wasmtime::Error ),

    /// I/O error reading the WASM binary.
    #[error( "Failed to read WASM data: {0}" )]
    FailedToReadWasm( std::io::Error ),

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

impl<I: InterfaceData, P: PluginData> std::fmt::Debug for LoadError<I, P> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        match self {
            Self::InvalidSocket( id ) => f.debug_tuple( "InvalidSocket" ).field( id ).finish(),
            Self::LoopDetected( id ) => f.debug_tuple( "LoopDetected" ).field( id ).finish(),
            Self::FailedCardinalityRequirements( c, n ) => f.debug_tuple( "FailedCardinalityRequirements" ).field( c ).field( n ).finish(),
            Self::CorruptedInterfaceManifest( e ) => f.debug_tuple( "CorruptedInterfaceManifest" ).field( e ).finish(),
            Self::CorruptedPluginManifest( e ) => f.debug_tuple( "CorruptedPluginManifest" ).field( e ).finish(),
            Self::FailedToLoadComponent( e ) => f.debug_tuple( "FailedToLoadComponent" ).field( e ).finish(),
            Self::FailedToReadWasm( e ) => f.debug_tuple( "FailedToReadWasm" ).field( e ).finish(),
            Self::FailedToLinkInterface( e ) => f.debug_tuple( "FailedToLinkInterface" ).field( e ).finish(),
            Self::FailedToLink( name, e ) => f.debug_tuple( "FailedToLink" ).field( name ).field( e ).finish(),
            Self::AlreadyHandled => f.debug_struct( "AlreadyHandled" ).finish(),
        }
    }
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside the [`Socket`] from [`PluginTreeHead::dispatch`] when a
/// function call fails at runtime.
///
/// [`Socket`]: crate::Socket
/// [`PluginTreeHead::dispatch`]: crate::PluginTreeHead::dispatch
#[derive( Error, Debug )]
pub enum DispatchError<InterfaceError: std::error::Error> {
    /// Failed to acquire lock on plugin instance (another call is in progress).
    #[error( "Deadlock" )] Deadlock,
    /// Failed to parse interface metadata during dispatch.
    #[error( "Wit parser error: {0}")] WitParserError( InterfaceError ),
    /// The specified interface path doesn't match any known interface.
    #[error( "Invalid Interface: {0}" )] InvalidInterface( String ),
    /// The specified function doesn't exist on the interface.
    #[error( "Invalid Function: {0}" )] InvalidFunction( String ),
    /// Function was expected to return a value but didn't.
    #[error( "Missing Response" )] MissingResponse,
    /// The WASM function threw an exception during execution.
    #[error( "Runtime Exception" )] RuntimeException( wasmtime::Error ),
    /// The provided arguments don't match the function signature.
    #[error( "Invalid Argument List" )] InvalidArgumentList,
    /// Failed to create a resource handle for cross-plugin transfer.
    #[error( "Resource Create Error: {0}" )] ResourceCreationError( #[from] ResourceCreationError ),
    /// Failed to receive a resource handle from another plugin.
    #[error( "Resource Receive Error: {0}" )] ResourceReceiveError( #[from] ResourceReceiveError ),
}

impl<T: std::error::Error> From<DispatchError<T>> for Val {
    fn from( error: DispatchError<T> ) -> Val { match error {
        DispatchError::Deadlock => Val::Variant( "deadlock".to_string(), None ),
        DispatchError::WitParserError( err ) => Val::Variant( "wit-parser-error".to_string(), Some( Box::new( Val::String( err.to_string() )))),
        DispatchError::InvalidInterface( package ) => Val::Variant( "invalid-interface".to_string(), Some( Box::new( Val::String( package )))),
        DispatchError::InvalidFunction( function ) => Val::Variant( "invalid-function".to_string(), Some( Box::new( Val::String( function )))),
        DispatchError::MissingResponse => Val::Variant( "missing-response".to_string(), None ),
        DispatchError::RuntimeException( exception ) => Val::Variant( "runtime-exception".to_string(), Some( Box::new( Val::String( exception.to_string() )))),
        DispatchError::InvalidArgumentList => Val::Variant( "invalid-argument-list".to_string(), None ),
        DispatchError::ResourceCreationError( err ) => err.into(),
        DispatchError::ResourceReceiveError( err ) => err.into(),
    }}
}

/// Result of a load operation that may have partial failures.
/// The `errors` field contains handled load failures
/// Convenience abstraction semantically equivalent to:
/// `( SocketMap, LoadResult<T, LoadError, LoadError> )`
pub(super) struct LoadResult<T, I: InterfaceData, P: PluginData + 'static> {
    pub socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    pub result: Result<T, LoadError<I, P>>,
    pub errors: Vec<LoadError<I, P>>,
}

#[allow( clippy::type_complexity )]
#[inline] pub(crate) fn load_plugin_tree<I, P>(
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
    engine: &Engine,
    default_linker: &Linker<P>,
    root: InterfaceId,
) -> PartialResult<( Arc<I>, Arc<LoadedSocket<P>> ), LoadError<I, P>, LoadError<I, P>>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{
    let socket_map = socket_map.into_iter()
        .map(|( socket_id, ( interface, plugins ))| ( socket_id, SocketState::Unprocessed( interface, plugins )))
        .collect();

    match load_socket( socket_map, engine, default_linker, root ) {
        LoadResult { socket_map: _, result: Ok(( interface, socket )), errors } => Ok((( interface, socket ), errors )),
        LoadResult { socket_map: _, result: Err( err ), errors } => Err(( err, errors ))
    }

}
