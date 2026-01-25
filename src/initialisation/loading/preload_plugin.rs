use std::collections::HashMap ;
use std::sync::{ Arc, RwLock };
use wasmtime::{ Engine, Store };
use wasmtime::component::{ Linker, ResourceType };

use crate::utils::Merge ;
use crate::InterfaceId ;
use super::{ PluginData, InterfaceData, FunctionData };
use super::{ Socket, PluginInstance, PluginContext, preload_socket, SocketState, PreloadResult, ResourceWrapper, PreloadError, LoadedSocket };



#[inline] pub fn preload_plugin<I, P, IE, PE>(
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<PluginContext<P>>,
    plugin: P,
) -> PreloadResult<PluginInstance<P>, I, P, IE, PE>
where
    IE: std::error::Error,
    PE: std::error::Error,
    I: InterfaceData<Error = IE>,
    P: PluginData<Error = PE> + Send + Sync,
{
    
    let socket_ids = match plugin.get_sockets() {
        Ok( ids ) => ids,
        Err( err ) => return PreloadResult { socket_map, result: Err( PreloadError::CorruptedPluginManifest( err ) ), errors: Vec::with_capacity( 0 ) }
    };

    let ( socket_map, sockets, errors ) = match preload_child_sockets( socket_ids, socket_map, engine, default_linker ) {
        PreloadResult { socket_map, result: Ok( sockets ), errors } => ( socket_map, sockets, errors ),
        PreloadResult { socket_map, result: Err( err ), errors } => return PreloadResult { socket_map, result: Err( err ), errors },
    };

    let linker: Linker<_> = match sockets.into_iter().try_fold(
        default_linker.clone(),
        | linker, ( interface, socket )| link_socket( linker, interface, socket ),
    ) {
        Ok( linker ) => linker,
        Err( err ) => return PreloadResult { socket_map, result: Err( err ), errors },
    };

    let component = match plugin.component( engine ) {
        Ok( component ) => component,
        Err( err ) => return PreloadResult { socket_map, result: Err( PreloadError::CorruptedPluginManifest( err )), errors }
    };

    let plugin_id = match plugin.get_id() {
        Ok( id ) => id.clone(),
        Err( err ) => return PreloadResult { socket_map, result: Err( PreloadError::CorruptedPluginManifest( err )), errors },
    };

    let host_data = PluginContext::new( plugin );
    let mut store = Store::new( engine, host_data );
    let instance = match linker.instantiate( &mut store, &component ) {
        Ok( instanace_pre ) => instanace_pre,
        Err( err ) => return PreloadResult { socket_map, result: Err( PreloadError::FailedToLoadComponent( err )), errors },
    };

    let lazy_plugin = PluginInstance {
        id: plugin_id,
        _component: component,
        store,
        instance,
    };

    PreloadResult { socket_map, result: Ok( lazy_plugin ), errors }

}

#[inline] fn preload_child_sockets<I, P, IE, PE>(
    socket_ids: impl IntoIterator<Item = InterfaceId>,
    socket_map: HashMap<InterfaceId, SocketState<I, P>>,
    engine: &Engine,
    default_linker: &Linker<PluginContext<P>>,
) -> PreloadResult<Vec<( Arc<I>, Arc<LoadedSocket<P>> )>, I, P, IE, PE>
where
    IE: std::error::Error,
    PE: std::error::Error,
    I: InterfaceData<Error = IE>,
    P: PluginData<Error = PE> + Send + Sync,
{

    match socket_ids.into_iter().try_fold(
        ( socket_map, Vec::<( _, _ )>::new(), Vec::<PreloadError<IE, PE>>::new() ),
        |( socket_map, sockets, errors ): ( _, Vec<_>, Vec<_> ), socket_id |
            match preload_socket( socket_map, engine, default_linker, socket_id.clone() ) {
                PreloadResult { socket_map, result: Ok( socket ), errors: new_errors } =>
                    Ok(( socket_map, sockets.merge(( socket.0, socket.1 )), errors.merge_all( new_errors ) )),
                PreloadResult { socket_map, result: Err( err ), errors: new_errors } =>
                    Err(( socket_map, err, errors.merge_all( new_errors ))),
            }
    ) {
        Ok(( socket_map, sockets, errors )) => PreloadResult { socket_map, result: Ok( sockets ), errors },
        Err(( socket_map, err, errors )) => PreloadResult { socket_map, result: Err( err ), errors },
    }

}

#[inline] fn link_socket<I, P, IE, PE>(
    mut linker: Linker<PluginContext<P>>,
    interface: Arc<I>,
    socket: Arc<Socket<RwLock<PluginInstance<P>>>>,
) -> Result<Linker<PluginContext<P>>, PreloadError<IE, PE>>
where
    IE: std::error::Error,
    PE: std::error::Error,
    I: InterfaceData<Error = IE>,
    P: PluginData<Error = PE> + Send + Sync,
{

    let package = match interface.get_package_name() {
        Ok( package ) => package,
        Err( err ) => return Err( PreloadError::CorruptedInterfaceManifest( err )),
    };
    let interface_ident = format!( "{}/root", package );

    let mut root = linker.root();
    let mut linker_instance = root.instance( &interface_ident ).map_err( PreloadError::FailedToLinkRootInterface )?;

    match interface.get_functions() {
        Ok( functions ) => functions.into_iter().try_for_each(| function: FunctionData | -> Result<(), PreloadError<IE, PE>> {
                
            let function_clone: FunctionData = function.clone();
            let interface_ident_clone = interface_ident.clone();
            let socket_arc_clone = Arc::clone( &socket );

            macro_rules! link {( $dispatch: expr ) => {
                linker_instance.func_new( function.name(), move | ctx, _ty, args, results | Ok(
                    results[0] = $dispatch( &socket_arc_clone, ctx, &interface_ident_clone, &function_clone, args )
                )).map_err(| err | PreloadError::FailedToLink( function.name().to_string(), err ) )
            }}

            match function.is_method() {
                false => link!( Socket::dispatch_function_all::<IE> ),
                true => link!( Socket::dispatch_function_method::<IE> ),
            }

        })?,
        Err( err ) => return Err( PreloadError::CorruptedInterfaceManifest( err )),
    };

    match interface.get_resources() {
        Ok( resources ) => resources.into_iter().try_for_each(| resource: String | linker_instance
            .resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper>>(), ResourceWrapper::drop )
            .map_err(| err | PreloadError::FailedToLink( resource, err ))
        )?,
        Err( err ) => return Err( PreloadError::CorruptedInterfaceManifest( err )),
    };

    Ok( linker )

}
