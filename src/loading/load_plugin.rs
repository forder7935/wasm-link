use std::collections::HashMap ;
use std::sync::{ Arc, RwLock };
use wasmtime::{ Engine, Store };
use wasmtime::component::{ Linker, ResourceType };

use crate::utils::Merge ;
use crate::InterfaceId ;
use super::{ PluginData, InterfaceData, FunctionData };
use super::{ Socket, PluginInstance, load_socket, SocketState, LoadResult, ResourceWrapper, LoadError, LoadedSocket };



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
        Ok( id ) => id.clone(),
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

#[inline] fn link_socket<I, P>(
    mut linker: Linker<P>,
    interface: &Arc<I>,
    socket: &Arc<Socket<RwLock<PluginInstance<P>>>>,
) -> Result<Linker<P>, LoadError<I, P>>
where
    I: InterfaceData,
    P: PluginData + Send + Sync,
{

    let package = match interface.get_package_name() {
        Ok( package ) => package,
        Err( err ) => return Err( LoadError::CorruptedInterfaceManifest( err )),
    };
    let interface_ident = format!( "{}/root", package );

    let mut root = linker.root();
    let mut linker_instance = root.instance( &interface_ident ).map_err( LoadError::FailedToLinkRootInterface )?;

    match interface.get_functions() {
        Ok( functions ) => functions.into_iter().try_for_each(| function | -> Result<(), LoadError<I, P>> {

            let function_clone: FunctionData = function.clone();
            let interface_ident_clone = interface_ident.clone();
            let socket_arc_clone = Arc::clone( socket );

            macro_rules! link {( $dispatch: expr ) => {
                linker_instance.func_new( function.name(), move | ctx, _ty, args, results | Ok(
                    results[0] = $dispatch( &socket_arc_clone, ctx, &interface_ident_clone, &function_clone, args )
                )).map_err(| err | LoadError::FailedToLink( function.name().to_string(), err ) )
            }}

            match function.is_method() {
                false => link!( Socket::dispatch_function_all::<I::Error> ),
                true => link!( Socket::dispatch_function_method::<I::Error> ),
            }

        })?,
        Err( err ) => return Err( LoadError::CorruptedInterfaceManifest( err )),
    }

    match interface.get_resources() {
        Ok( resources ) => resources.into_iter().try_for_each(| resource | linker_instance
            .resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper>>(), ResourceWrapper::drop )
            .map_err(| err | LoadError::FailedToLink( resource.clone(), err ))
        )?,
        Err( err ) => return Err( LoadError::CorruptedInterfaceManifest( err )),
    }

    Ok( linker )

}
