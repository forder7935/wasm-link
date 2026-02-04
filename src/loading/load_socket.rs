use std::collections::HashMap ;
use std::sync::{ Arc, Mutex };
use pipe_trait::Pipe;
use wasmtime::Engine;
use wasmtime::component::Linker ;

use crate::utils::Merge ;
use crate::interface::{ Binding, Cardinality };
use crate::plugin::{ Plugin, PluginContext } ;
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use super::{ LoadResult, LoadError, LoadedSocket };
use super::load_plugin::load_plugin ;



#[derive( Debug, Default )]
pub(super) enum SocketState<BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq,
    PluginId: Clone,
    Ctx: PluginContext + 'static,
{
    Unprocessed( Binding<BindingId>, Vec<Plugin<PluginId, BindingId, Ctx>> ),
    Loaded( Arc<Binding<BindingId>>, Arc<LoadedSocket<PluginId, Ctx>> ),
    Failed,
    #[default] Borrowed,
}

impl<BindingId, PluginId, Ctx> From<( Binding<BindingId>, Vec<Plugin<PluginId, BindingId, Ctx>> )> for SocketState<BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq,
    PluginId: Clone,
    Ctx: PluginContext + 'static,
{
    fn from(( interface, plugins ): ( Binding<BindingId>, Vec<Plugin<PluginId, BindingId, Ctx>> )) -> Self {
        Self::Unprocessed( interface, plugins )
    }
}

#[allow( clippy::type_complexity )]
pub(super) fn load_socket<BindingId, PluginId, Ctx>(
    mut socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    socket_id: BindingId,
) -> LoadResult<( Arc<Binding<BindingId>>, Arc<LoadedSocket<PluginId, Ctx>> ), BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    // NOTE: do not forget to add the entry back if it's already loaded
    let Some( socket_plugins ) = socket_map.insert( socket_id.clone(), SocketState::Borrowed ) else {
        return LoadResult {
            socket_map,
            result: Err( LoadError::InvalidSocket( socket_id )),
            errors: Vec::with_capacity( 0 )
        };
    };

    match socket_plugins {
        SocketState::Borrowed => LoadResult {
            socket_map,
            result: Err( LoadError::LoopDetected( socket_id )),
            errors: Vec::with_capacity( 0 )
        },
        SocketState::Failed => LoadResult {
            socket_map,
            result: Err( LoadError::AlreadyHandled ),
            errors: Vec::with_capacity( 0 )
        },
        SocketState::Loaded( interface, plugins ) => {
            let interface_arc = Arc::clone( &interface );
            let plugins_arc = Arc::clone( &plugins );
            // NOTE: readding the entry since it was taken out to gain ownership
            socket_map.insert( socket_id.clone(), SocketState::Loaded( interface, plugins ));
            LoadResult { socket_map, result: Ok(( interface_arc, plugins_arc )), errors: Vec::with_capacity( 0 ) }
        },
        SocketState::Unprocessed( interface, plugins ) => {
            let LoadResult { mut socket_map, result, errors } = load_socket_unprocessed(
                socket_map, interface, plugins, engine, default_linker );
            match result {
                Ok(( interface, plugins )) => {
                    let interface = Arc::new( interface );
                    let plugins = Arc::new( plugins.map_mut( Mutex::new ));
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

#[inline]
#[allow( clippy::type_complexity )]
fn load_socket_unprocessed<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    interface: Binding<BindingId>,
    plugins: Vec<Plugin<PluginId, BindingId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
) -> LoadResult<( Binding<BindingId>, Socket<PluginInstance<PluginId, Ctx>, PluginId> ), BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    match interface.cardinality() {
        Cardinality::AtMostOne => load_most_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugin_opt ) => LoadResult { socket_map, result: Ok(( interface, Socket::AtMostOne( plugin_opt ))), errors },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        Cardinality::ExactlyOne => load_exact_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugin ) => LoadResult { socket_map, result: Ok(( interface, Socket::ExactlyOne( plugin ))), errors },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        Cardinality::AtLeastOne => load_at_least_one( socket_map, engine, default_linker, plugins )
            .pipe(| LoadResult { socket_map, result, errors } | match result {
                Ok( plugins ) => LoadResult {
                    socket_map,
                    result: Ok(( interface, Socket::AtLeastOne( plugins.into_iter()
                        .map(| plugin: PluginInstance<PluginId, Ctx> | ( plugin.id().clone(), plugin ))
                        .collect()
                    ))),
                    errors,
                },
                Err( err ) => LoadResult { socket_map, result: Err( err ), errors },
            }),
        Cardinality::Any => load_any( socket_map, engine, default_linker, plugins )
            .pipe(|( socket_map, plugins, errors )| {
                let plugins = plugins.into_iter()
                    .map(| plugin: PluginInstance<PluginId, Ctx> | ( plugin.id().clone(), plugin ))
                    .collect();
                LoadResult {
                    socket_map,
                    result: Ok(( interface, Socket::Any( plugins ) )),
                    errors,
                }
            }),
    }
}

#[inline]
fn load_most_one<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    mut plugins: Vec<Plugin<PluginId, BindingId, Ctx>>,
) -> LoadResult<Option<PluginInstance<PluginId, Ctx>>, BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    match plugins.pop() {
        Option::None => LoadResult { socket_map, result: Ok( None ), errors: Vec::with_capacity( 0 ) },
        Some( plugin ) => match plugins.pop() {
            Option::None => match load_plugin( socket_map, engine, default_linker, plugin ) {
                LoadResult { socket_map, result: Ok( plugin ), errors }
                    => LoadResult { socket_map, result: Ok( Some( plugin )), errors },
                LoadResult { socket_map, result: Err( err ), errors }
                    => LoadResult { socket_map, result: Ok( None ), errors: errors.merge( err ) },
            },
            Some( _ ) => LoadResult {
                socket_map,
                result: Err( LoadError::FailedCardinalityRequirements( Cardinality::AtMostOne, plugins.len() +2 )),
                errors: Vec::with_capacity( 0 )
            },
        }
    }
}

#[inline]
fn load_exact_one<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    mut plugins: Vec<Plugin<PluginId, BindingId, Ctx>>,
) -> LoadResult<PluginInstance<PluginId, Ctx>, BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    match plugins.pop() {
        Option::None => LoadResult {
            socket_map,
            result: Err( LoadError::FailedCardinalityRequirements( Cardinality::ExactlyOne, 0 )),
            errors: Vec::with_capacity( 0 )
        },
        Some( plugin ) => match plugins.pop() {
            Option::None => load_plugin( socket_map, engine, default_linker, plugin ),
            Some( _ ) => LoadResult {
                socket_map,
                result: Err( LoadError::FailedCardinalityRequirements( Cardinality::ExactlyOne, plugins.len() +2 )),
                errors: Vec::with_capacity( 0 )
            },
        }
    }

}

#[inline]
fn load_at_least_one<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    plugins: Vec<Plugin<PluginId, BindingId, Ctx>>,
) -> LoadResult<Vec<PluginInstance<PluginId, Ctx>>, BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    if plugins.is_empty() { return LoadResult {
        socket_map,
        result: Err( LoadError::FailedCardinalityRequirements( Cardinality::AtLeastOne, 0 )),
        errors: Vec::with_capacity( 0 ),
    }}

    let ( socket_map, plugins, errors ) = load_any( socket_map, engine, default_linker, plugins );

    if plugins.is_empty() { return LoadResult {
        socket_map,
        result: Err( LoadError::FailedCardinalityRequirements( Cardinality::AtLeastOne, 0 )),
        errors,
    }}

    LoadResult { socket_map, result: Ok( plugins ), errors }

}

#[inline]
#[allow( clippy::type_complexity )]
fn load_any<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    plugins: Vec<Plugin<PluginId, BindingId, Ctx>>,
) -> (
    HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    Vec<PluginInstance<PluginId, Ctx>>,
    Vec<LoadError<BindingId>>,
) where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    let ( socket_map, plugins, errors ) = plugins.into_iter().fold(
        ( socket_map, Vec::new(), Vec::new()),
        |( socket_map, plugins, errors ), plugin | match load_plugin( socket_map, engine, default_linker, plugin ) {
            LoadResult { socket_map, result: Ok( plugin ), errors: new_errors } => ( socket_map, plugins.merge( plugin ), errors.merge_all( new_errors )),
            LoadResult { socket_map, result: Err( err ), errors: new_errors } => ( socket_map, plugins, errors.merge_all( new_errors ).merge( err ))
        }
    );

    ( socket_map, plugins, errors )

}
