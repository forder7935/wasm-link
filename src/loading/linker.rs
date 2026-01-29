use std::sync::{ Arc, RwLock };
use wasmtime::{ AsContextMut, StoreContextMut };
use wasmtime::component::{ Linker, ResourceType, Val };

use crate::interface::{ InterfaceData, FunctionData, ReturnKind };
use crate::plugin::{ PluginId, PluginData };
use crate::socket::Socket ;
use crate::plugin_instance::PluginInstance ;
use super::{ LoadError, DispatchError, ResourceWrapper, ResourceCreationError };



pub type LoadedSocket<P> = Socket<RwLock<PluginInstance<P>>> ;

#[inline] pub fn link_socket<I, P>(
    mut linker: Linker<P>,
    interface: &Arc<I>,
    socket: &Arc<LoadedSocket<P>>,
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
    let mut linker_instance = root.instance( &interface_ident ).map_err( LoadError::FailedToLinkInterface )?;

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
                false => link!( dispatch_all::<P, I::Error> ),
                true => link!( dispatch_method::<P, I::Error> ),
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

/// Dispatches a non-method function call to all plugins
pub(crate) fn dispatch_all<P, E>(
    socket: &Arc<LoadedSocket<P>>,
    mut ctx: StoreContextMut<P>,
    interface_path: &str,
    function: &FunctionData,
    data: &[Val],
) -> Val
where
    P: PluginData,
    E: std::error::Error,
{
    debug_assert!( !function.is_method() );
    socket.map(| plugin | Val::Result( match dispatch_of::<P, E>( &mut ctx, plugin, interface_path, function, data ) {
        Ok( val ) => Ok( Some( Box::new( val ))),
        Err( err ) => Err( Some( Box::new( err.into() ))),
    })).into()
}

/// Dispatches a method function call, routing to the correct plugin.
pub(crate) fn dispatch_method<P, E>(
    socket: &Arc<LoadedSocket<P>>,
    ctx: StoreContextMut<P>,
    interface_path: &str,
    function: &FunctionData,
    data: &[Val],
) -> Val
where
    P: PluginData,
    E: std::error::Error,
{
    debug_assert!( function.is_method() );
    Val::Result( match route_method::<P, E>( socket, ctx, interface_path, function, data ) {
        Ok( val ) => Ok( Some( Box::new( val ))),
        Err( err ) => Err( Some( Box::new( err.into() ))),
    })
}

#[inline] fn dispatch_of<P, E>(
    ctx: &mut StoreContextMut<P>,
    plugin: &RwLock<PluginInstance<P>>,
    interface_path: &str,
    function: &FunctionData,
    data: &[Val],
) -> Result<Val, DispatchError<E>>
where
    P: PluginData,
    E: std::error::Error,
{

    let mut lock = plugin.write().map_err(|_| DispatchError::Deadlock )?;
    let result = lock.dispatch( interface_path, function.name(), function.has_return(), data )?;

    Ok( match function.return_kind() {
        ReturnKind::Void | ReturnKind::AssumeNoResources => result,
        ReturnKind::MayContainResources => wrap_resources( result, *lock.id(), ctx )?,
    })
}

#[inline] fn route_method<P, E>(
    socket: &LoadedSocket<P>,
    mut ctx: StoreContextMut<P>,
    interface_path: &str,
    function: &FunctionData,
    data: &[Val],
) -> Result<Val, DispatchError<E>>
where
    P: PluginData,
    E: std::error::Error,
{

    let handle = match data.first() {
        Some( Val::Resource( handle )) => Ok( handle ),
        _ => Err( DispatchError::InvalidArgumentList ),
    }?;

    let resource = ResourceWrapper::from_handle( *handle, &mut ctx )?;
    let plugin = socket.get( resource.plugin_id )
        .map_err(|_| DispatchError::Deadlock )?
        .ok_or( DispatchError::InvalidArgumentList )?;

    let mut data = Vec::from( data );
    data[0] = Val::Resource( resource.resource_handle );

    dispatch_of( &mut ctx, plugin, interface_path, function, &data )

}

fn wrap_resources( val: Val, plugin_id: PluginId, store: &mut impl AsContextMut ) -> Result<Val, ResourceCreationError> {
    Ok( match val {
        Val::Bool( _ )
        | Val::S8( _ ) | Val::S16( _ ) | Val::S32( _ ) | Val::S64( _ )
        | Val::U8( _ ) | Val::U16( _ ) | Val::U32( _ ) | Val::U64( _ )
        | Val::Float32( _ ) | Val::Float64( _ )
        | Val::Char( _ )
        | Val::String( _ )
        | Val::Enum( _ )
        | Val::Flags( _ )
        | Val::Variant( _, Option::None )
        | Val::Option( None )
        | Val::Result( Ok( Option::None )) | Val::Result( Err( Option::None )) => val,
        Val::List( list ) => Val::List( list.into_iter().map(| item | wrap_resources( item, plugin_id, store )).collect::<Result<_,_>>()? ),
        Val::Record( entries ) => Val::Record( entries.into_iter().map(|( key, value )| Ok(( key, wrap_resources( value, plugin_id, store )?)) ).collect::<Result<_,_>>()? ),
        Val::Tuple( list ) => Val::Tuple( list.into_iter().map(| item | wrap_resources( item, plugin_id, store )).collect::<Result<_,_>>()? ),
        Val::Variant( variant, Some( data_box )) => Val::Variant( variant, Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
        Val::Option( Some( data_box )) => Val::Option( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
        Val::Result( Ok( Some( data_box ))) => Val::Result( Ok( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
        Val::Result( Err( Some( data_box ))) => Val::Result( Err( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
        Val::Resource( handle ) => Val::Resource( ResourceWrapper::new( plugin_id, handle ).attach( store )? ),
        Val::Future( _ ) => unimplemented!( "'Val::Future' is not yet supported" ),
        Val::Stream( _ ) => unimplemented!( "'Val::Stream' is not yet supported" ),
        Val::ErrorContext( _ ) => unimplemented!( "'Val::ErrorContext' is not yet supported" ),
    })
}
