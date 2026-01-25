use std::sync::Arc ;
use std::collections::HashMap ;
use thiserror::Error ;
use wasmtime::Engine;
use wasmtime::component::Linker ;

use crate::InterfaceId ;
use crate::utils::PartialResult ;
use super::{ InterfaceData, PluginData, InterfaceCardinality };
use super::{ load_socket, SocketState, LoadedSocket };



#[derive( Error, Debug )]
pub enum LoadError<
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

/// Result of a load operation that may have partial failures.
/// The `errors` field contains handled load failures
/// Convenience abstraction semantically equivalent to:
/// `( SocketMap, LoadResult<T, LoadError, LoadError> )`
#[derive(Debug)]
pub(super) struct LoadResult<T, I: InterfaceData, P: PluginData + 'static> {
    pub socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    pub result: Result<T, LoadError<I::Error, P::Error>>,
    pub errors: Vec<LoadError<I::Error, P::Error>>,
}

#[inline] pub(crate) fn load_plugin_tree<I, P>(
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
    engine: &Engine,
    default_linker: &Linker<P>,
    root: InterfaceId,
) -> PartialResult<( Arc<I>, Arc<LoadedSocket<P>> ), LoadError<I::Error, P::Error>, LoadError<I::Error, P::Error>>
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
