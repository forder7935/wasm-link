use std::collections::HashMap ;
use std::sync::Arc ;
use wasmtime::{ Engine, Store };
use wasmtime::component::Linker ;

use crate::utils::Merge ;
use crate::interface::{ InterfaceId, InterfaceData };
use crate::plugin::PluginData ;
use crate::plugin_instance::PluginInstance ;
use super::{ LoadResult, LoadError, LoadedSocket };
use super::load_socket::{ SocketState, load_socket };
use super::link_socket ;



#[inline] pub fn load_plugin<I, P>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
    plugin: P,
) -> LoadResult<PluginInstance<P>, I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    let socket_ids = match plugin.get_sockets() {
        Ok( ids ) => ids,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::CorruptedPluginManifest( err ) ), errors: Vec::with_capacity( 0 ) }
    };

    let ( socket_map, sockets, errors ) = match load_child_sockets( socket_ids, socket_map, engine, default_linker ) {
        LoadResult { socket_map, result: Ok( sockets ), errors } => ( socket_map, sockets, errors ),
        LoadResult { socket_map, result: Err( err ), errors } => return LoadResult { socket_map, result: Err( err ), errors },
    };

    let linker: Linker<_> = match sockets.iter().try_fold(
        default_linker.clone(),
        | linker, ( interface, socket )| link_socket( linker, interface, socket ),
    ) {
        Ok( linker ) => linker,
        Err( err ) => return LoadResult { socket_map, result: Err( err ), errors },
    };

    let component = match plugin.component( engine ) {
        Ok( component ) => component,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::CorruptedPluginManifest( err )), errors }
    };

    let plugin_id = match plugin.get_id() {
        Ok( id ) => *id,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::CorruptedPluginManifest( err )), errors },
    };

    let mut store = Store::new( engine, plugin );
    let instance = match linker.instantiate( &mut store, &component ) {
        Ok( instanace_pre ) => instanace_pre,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::FailedToLoadComponent( err )), errors },
    };

    let lazy_plugin = PluginInstance {
        id: plugin_id,
        _component: component,
        store,
        instance,
    };

    LoadResult { socket_map, result: Ok( lazy_plugin ), errors }

}

#[allow( clippy::type_complexity )]
#[inline] fn load_child_sockets<'a, I, P>(
    socket_ids: impl IntoIterator<Item = &'a InterfaceId>,
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<P>,
) -> LoadResult<Vec<( Arc<I>, Arc<LoadedSocket<P>> )>, I, P>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{
    match socket_ids.into_iter().try_fold(
        ( socket_map, Vec::<( _, _ )>::new(), Vec::<LoadError<I, P>>::new() ),
        |( socket_map, sockets, errors ): ( _, Vec<_>, Vec<_> ), socket_id |
            match load_socket( socket_map, engine, default_linker, *socket_id ) {
                LoadResult { socket_map, result: Ok( socket ), errors: new_errors } =>
                    Ok(( socket_map, sockets.merge(( socket.0, socket.1 )), errors.merge_all( new_errors ) )),
                LoadResult { socket_map, result: Err( err ), errors: new_errors } =>
                    Err(( socket_map, err, errors.merge_all( new_errors ))),
            }
    ) {
        Ok(( socket_map, sockets, errors )) => LoadResult { socket_map, result: Ok( sockets ), errors },
        Err(( socket_map, err, errors )) => LoadResult { socket_map, result: Err( err ), errors },
    }
}
