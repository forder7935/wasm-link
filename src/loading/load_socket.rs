use std::collections::HashMap ;
use std::sync::{ Arc, RwLock };
use pipe_trait::Pipe;
use wasmtime::Engine;
use wasmtime::component::Linker ;

use crate::utils::{ MapScanTrait, Merge };
use crate::interface::{ InterfaceId, InterfaceData, InterfaceCardinality };
use crate::plugin::PluginData ;
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use super::{ LoadResult, LoadError, LoadedSocket };
use super::load_plugin::load_plugin ;



#[derive( Debug, Default )]
pub(super) enum SocketState<I: InterfaceData, P: PluginData + 'static> {
    Unprocessed( I, Vec<P> ),
    Loaded( Arc<I>, Arc<LoadedSocket<P>> ),
    Failed,
    #[default] Borrowed,
}
impl<I: InterfaceData, P: PluginData> From<( I, Vec<P> )> for SocketState<I, P> {
    fn from(( interface, plugins ): ( I, Vec<P> )) -> Self { Self::Unprocessed( interface, plugins )}
}

#[allow( clippy::type_complexity )]
pub(super) fn load_socket<I, P>(
    mut socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    socket_id: InterfaceId
) -> LoadResult<( Arc<I>, Arc<LoadedSocket<P>> ), I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    // NOTE: do not forget to add the entry back if it's already loaded
    let Some( socket_plugins ) = socket_map.insert( socket_id, SocketState::Borrowed ) else {
        return LoadResult { socket_map, result: Err( LoadError::InvalidSocket( socket_id )), errors: Vec::with_capacity( 0 )};
    };

    match socket_plugins {
        SocketState::Borrowed => LoadResult { socket_map, result: Err( LoadError::LoopDetected( socket_id )), errors: Vec::with_capacity( 0 ) },
        SocketState::Failed => LoadResult { socket_map, result: Err( LoadError::AlreadyHandled ), errors: Vec::with_capacity( 0 ) },
        SocketState::Loaded( interface, plugins ) => {
            let interface_arc = Arc::clone( &interface );
            let plugins_arc = Arc::clone( &plugins );
            // NOTE: readding entry since it was taken out to gain ownership
            socket_map.insert( socket_id, SocketState::Loaded( interface, plugins ));
            LoadResult { socket_map, result: Ok(( interface_arc, plugins_arc )), errors: Vec::with_capacity( 0 ) }
        },
        SocketState::Unprocessed( interface, plugins ) => {
            let LoadResult { mut socket_map, result, errors } = load_socket_unprocessed(
                socket_map, interface, plugins, engine, default_linker );
            match result {
                Ok(( interface, plugins )) => {
                    let interface = Arc::new( interface );
                    let plugins = Arc::new( plugins.map_mut( RwLock::new ));
                    socket_map.insert( socket_id, SocketState::Loaded( Arc::clone( &interface ), Arc::clone( &plugins )));
                    LoadResult { socket_map, result: Ok(( interface, plugins )), errors }
                },
                Err( err ) => {
                    socket_map.insert( socket_id, SocketState::Failed );
                    LoadResult { socket_map, result: Err( err ), errors }
                }
            }
        }
    }

}

#[inline] fn load_socket_unprocessed<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    interface: I,
    plugins: Vec<P>,
    engine: &Engine,
    default_linker: &Linker<P>,
) -> LoadResult<( I, Socket<PluginInstance<P>> ), I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    let cardinality = match interface.cardinality() {
        Ok( cardinality ) => cardinality,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::CorruptedInterfaceManifest( err )), errors: Vec::with_capacity( 0 ) },
    };

    match cardinality {
        InterfaceCardinality::AtMostOne => load_most_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugin_opt ) => LoadResult { socket_map, result: Ok(( interface, Socket::AtMostOne( plugin_opt ))), errors },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        InterfaceCardinality::ExactlyOne => load_exact_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugin ) => LoadResult { socket_map, result: Ok(( interface, Socket::ExactlyOne( plugin ))), errors },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        InterfaceCardinality::AtLeastOne => load_at_least_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugins ) => LoadResult {
                    socket_map,
                    result: Ok(( interface, Socket::AtLeastOne( plugins.into_iter().map(| plugin: PluginInstance<P> | ( *plugin.id(), plugin )).collect() ) )),
                    errors,
                },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        InterfaceCardinality::Any => load_any( socket_map, engine, default_linker, plugins )
            .pipe(|( socket_map, plugins, errors )| {
                let plugins = plugins.into_iter()
                    .map(| plugin: PluginInstance<P> | ( *plugin.id(), plugin ))
                    .collect();
                LoadResult {
                    socket_map,
                    result: Ok(( interface, Socket::Any( plugins ) )),
                    errors,
                }
            }),
    }
}

#[inline] fn load_most_one<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    mut plugins: Vec<P>,
) -> LoadResult<Option<PluginInstance<P>>, I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{
    match plugins.pop() {
        Option::None => LoadResult { socket_map, result: Ok( None ), errors: Vec::with_capacity( 0 ) },
        Some( plugin ) => match plugins.pop() {
            Option::None => match load_plugin( socket_map, engine, default_linker, plugin ) {
                LoadResult { socket_map, result: Ok( plugin ), errors } => LoadResult { socket_map, result: Ok( Some( plugin )), errors },
                LoadResult { socket_map, result: Err( err ), errors } => LoadResult { socket_map, result: Ok( None ), errors: errors.merge( err ) },
            },
            Some( _ ) => LoadResult {
                socket_map,
                result: Err( LoadError::FailedCardinalityRequirements( InterfaceCardinality::AtMostOne, plugins.len() +2 )),
                errors: Vec::with_capacity( 0 )
            },
        }
    }
}

#[inline] fn load_exact_one<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    mut plugins: Vec<P>,
) -> LoadResult<PluginInstance<P>, I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    match plugins.pop() {
        Option::None => LoadResult {
            socket_map,
            result: Err( LoadError::FailedCardinalityRequirements( InterfaceCardinality::ExactlyOne, 0 )),
            errors: Vec::with_capacity( 0 )
        },
        Some( plugin ) => match plugins.pop() {
            Option::None => load_plugin( socket_map, engine, default_linker, plugin ),
            Some( _ ) => LoadResult {
                socket_map,
                result: Err( LoadError::FailedCardinalityRequirements( InterfaceCardinality::ExactlyOne, plugins.len() +2 )),
                errors: Vec::with_capacity( 0 )
            },
        }
    }

}

#[inline] fn load_at_least_one<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    plugins: Vec<P>,
) -> LoadResult<Vec<PluginInstance<P>>, I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    if plugins.is_empty() { return LoadResult {
        socket_map,
        result: Err( LoadError::FailedCardinalityRequirements( InterfaceCardinality::AtLeastOne, 0 )),
        errors: Vec::with_capacity( 0 ),
    }}

    let ( socket_map, plugins, errors ) = load_any( socket_map, engine, default_linker, plugins );

    if plugins.is_empty() { return LoadResult {
        socket_map,
        result: Err( LoadError::FailedCardinalityRequirements( InterfaceCardinality::AtLeastOne, 0 )),
        errors,
    }}

    LoadResult { socket_map, result: Ok( plugins ), errors }

}

#[allow( clippy::type_complexity )]
#[inline] fn load_any<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    plugins: Vec<P>,
) -> (
    HashMap<InterfaceId, SocketState<I, P>>,
    Vec<PluginInstance<P>>,
    Vec<LoadError<I, P>>,
) where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    let (( plugins, errors ), socket_map ) = plugins.into_iter().map_scan(
        socket_map,
        | plugin, socket_map | match load_plugin( socket_map, engine, default_linker, plugin ) {
            LoadResult { socket_map, result: Ok( plugin ), errors } => (( Some( plugin ), errors ), socket_map ),
            LoadResult { socket_map, result: Err( err ), errors } => (( None, errors.merge( err )), socket_map )
        }
    ).unzip_get_state::<Vec<_>, Vec<_>>();

    let plugins = plugins.into_iter().flatten().collect::<Vec<_>>();
    let errors = errors.into_iter().flatten().collect::<Vec<_>>();

    ( socket_map, plugins, errors )

}
