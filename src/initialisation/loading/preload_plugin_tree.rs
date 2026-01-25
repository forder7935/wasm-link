use std::sync::Arc ;
use std::collections::HashMap ;
use thiserror::Error ;
use wasmtime::Engine;
use wasmtime::component::Linker ;

use crate::InterfaceId ;
use crate::utils::PartialResult ;
use super::{ InterfaceData, PluginData, InterfaceCardinality };
use super::{ PluginContext, preload_socket, SocketState, LoadedSocket };



#[derive( Error, Debug )]
pub enum PreloadError<
    InterfaceError: std::error::Error,
    PluginError: std::error::Error,
> {
    
    #[error( "Invalid socket: {0}" )]
    InvalidSocket( InterfaceId ),
    
    #[error( "Loop detected loading: '{0}'" )]
    LoopDetected( InterfaceId ),
    
    #[error( "Failed to meet cardinality requirements: {0}, found {1}" )]
    FailedCardinalityRequirements( InterfaceCardinality, usize ),
    
    #[error( "Corrupted interface manifest: {0}" )]
    CorruptedInterfaceManifest( InterfaceError ),

    #[error( "Corrupted plugin manifest: {0}" )]
    CorruptedPluginManifest( PluginError ),
    
    #[error( "Failed to load component: {0}" )]
    FailedToLoadComponent( wasmtime::Error ),

    #[error( "Failed to read WASM data: {0}" )]
    FailedToReadWasm( std::io::Error ),

    #[error( "Failed to link root interface: {0}" )]
    FailedToLinkRootInterface( wasmtime::Error ),

    #[error( "Failed to link function '{0}': {1}" )]
    FailedToLink( String, wasmtime::Error ),

    #[error( "Handled failure" )]
    AlreadyHandled,

}

/// Result of a preload operation that may have partial failures.
/// The `errors` field contains handled preload failures
/// Convinience abstraction semantically equivalent to:
/// `( SocketMap, PreloadResult<T, PreloadError, PreloadError> )`
#[derive(Debug)]
pub(super) struct PreloadResult<T, I: InterfaceData, P: PluginData + 'static, IE: std::error::Error, PE: std::error::Error> {
    pub socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    pub result: Result<T, PreloadError<IE, PE>>,
    pub errors: Vec<PreloadError<IE, PE>>,
}

#[inline] pub(crate) fn preload_plugin_tree<I, P, IE, PE>(
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
    engine: &Engine,
    default_linker: &Linker<PluginContext<P>>,
    root: InterfaceId,
) -> PartialResult<( Arc<I>, Arc<LoadedSocket<P>> ), PreloadError<IE, PE>, PreloadError<IE, PE>>
where 
    IE: std::error::Error,
    PE: std::error::Error,
    I: InterfaceData<Error = IE>,
    P: PluginData<Error = PE> + Send + Sync,
{
    let socket_map = socket_map.into_iter()
        .map(|( socket_id, ( interface, plugins ))| ( socket_id, SocketState::Unprocessed( interface, plugins )))
        .collect();

    match preload_socket( socket_map, engine, default_linker, root ) {
        PreloadResult { socket_map: _, result: Ok(( interface, socket )), errors } => Ok((( interface, socket ), errors )),
        PreloadResult { socket_map: _, result: Err( err ), errors } => Err(( err, errors ))
    }

}
