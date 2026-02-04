use std::collections::HashMap ;
use std::sync::Arc ;
use wasmtime::{ Engine, Store };
use wasmtime::component::Linker ;

use crate::utils::Merge ;
use crate::interface::Binding ;
use crate::plugin::{ Plugin, PluginContext } ;
use crate::plugin_instance::PluginInstance ;
use super::{ LoadResult, LoadError, LoadedSocket };
use super::load_socket::{ SocketState, load_socket };
use super::link_socket ;



#[inline]
pub fn load_plugin<BindingId, PluginId, Ctx>(
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
    plugin: Plugin<PluginId, BindingId, Ctx>,
) -> LoadResult<PluginInstance<PluginId, Ctx>, BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    let ( id, socket_ids, component, context ) = plugin.into_parts();

    let ( socket_map, sockets, errors ): ( _, _, _ ) = match load_child_sockets( socket_ids.iter(), socket_map, engine, default_linker ) {
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

    let mut store = Store::new( engine, context );
    let instance = match linker.instantiate( &mut store, &component ) {
        Ok( instance ) => instance,
        Err( err ) => return LoadResult { socket_map, result: Err( LoadError::FailedToLoadComponent( err )), errors },
    };

    let lazy_plugin = PluginInstance {
        id,
        store,
        instance,
    };

    LoadResult { socket_map, result: Ok( lazy_plugin ), errors }

}

#[inline]
#[allow( clippy::type_complexity )]
fn load_child_sockets<'a, BindingId, PluginId, Ctx>(
    socket_ids: impl IntoIterator<Item = &'a BindingId>,
    socket_map: HashMap<BindingId, SocketState<BindingId, PluginId, Ctx>>,
    engine: &Engine,
    default_linker: &Linker<Ctx>,
) -> LoadResult<Vec<( Arc<Binding<BindingId>>, Arc<LoadedSocket<PluginId, Ctx>> )>, BindingId, PluginId, Ctx>
where
    BindingId: 'a + Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    match socket_ids.into_iter().try_fold(
        ( socket_map, Vec::<( _, _ )>::new(), Vec::<LoadError<BindingId>>::new() ),
        |( socket_map, sockets, errors ): ( _, Vec<_>, Vec<_> ), socket_id |
            match load_socket( socket_map, engine, default_linker, socket_id.clone() ) {
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

