use std::sync::{ Arc, Mutex };
use wasmtime::StoreContextMut ;
use wasmtime::component::{ Linker, LinkerInstance, ResourceType, Val };

use crate::{ Binding, Interface, Function, ReturnKind, Socket, PluginContext, DispatchError };
use crate::plugin_instance::PluginInstance ;
use super::{ LoadError, ResourceWrapper };



pub type LoadedSocket<PluginId, Ctx> = Socket<Mutex<PluginInstance<PluginId, Ctx>>, PluginId> ;

#[inline]
pub fn link_socket<BindingId, PluginId, Ctx>(
    mut linker: Linker<Ctx>,
    binding: &Arc<Binding<BindingId>>,
    socket: &Arc<LoadedSocket<PluginId, Ctx>>,
) -> Result<Linker<Ctx>, LoadError<BindingId>>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{
    let mut root = linker.root();

    binding.interfaces().iter().try_for_each(| interface | {
        let interface_ident = format!( "{}/{}", binding.package_name(), interface.name() );
        let linker_instance = root.instance( &interface_ident ).map_err( LoadError::FailedToLinkInterface )?;
        let _ = link_socket_interface( linker_instance, interface, &interface_ident, socket )?;
        Ok(())
    })?;

    Ok( linker )
}

#[inline]
fn link_socket_interface<'a, BindingId, PluginId, Ctx>(
    mut linker_instance: LinkerInstance<'a, Ctx>,
    interface: &Interface,
    interface_ident: &str,
    socket: &Arc<LoadedSocket<PluginId, Ctx>>,
) -> Result<LinkerInstance<'a, Ctx>, LoadError<BindingId>>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext + 'static,
{

    interface.functions().iter().try_for_each(| function | -> Result<(), LoadError<BindingId>> {

        let function_clone = function.clone();
        let interface_ident_clone = interface_ident.to_string();
        let socket_arc_clone = Arc::clone( socket );

        macro_rules! link {( $dispatch: expr ) => {
            linker_instance.func_new( function.name(), move | ctx, _ty, args, results | Ok(
                results[0] = $dispatch( &socket_arc_clone, ctx, &interface_ident_clone, &function_clone, args )
            )).map_err(| err | LoadError::FailedToLink( function.name().to_string(), err ) )
        }}

        match function.is_method() {
            false => link!( dispatch_all ),
            true => link!( dispatch_method ),
        }

    })?;

    interface.resources().iter().try_for_each(| resource | linker_instance
        .resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper<PluginId>>>(), ResourceWrapper::<PluginId>::drop )
        .map_err(| err | LoadError::FailedToLink( resource.clone(), err ))
    )?;

    Ok( linker_instance )

}

/// Dispatches a non-method function call to all plugins
pub(crate) fn dispatch_all<PluginId, Ctx>(
    socket: &Arc<LoadedSocket<PluginId, Ctx>>,
    mut ctx: StoreContextMut<Ctx>,
    interface_path: &str,
    function: &Function,
    data: &[Val],
) -> Val
where
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext,
{
    debug_assert!( !function.is_method() );
    socket.map(| plugin | Val::Result( match dispatch_of::<PluginId, Ctx>( &mut ctx, plugin, interface_path, function, data ) {
        Ok( val ) => Ok( Some( Box::new( val ))),
        Err( err ) => Err( Some( Box::new( err.into() ))),
    })).into()
}

/// Dispatches a method function call, routing to the correct plugin.
pub(crate) fn dispatch_method<PluginId, Ctx>(
    socket: &Arc<LoadedSocket<PluginId, Ctx>>,
    ctx: StoreContextMut<Ctx>,
    interface_path: &str,
    function: &Function,
    data: &[Val],
) -> Val
where
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext,
{
    debug_assert!( function.is_method() );
    Val::Result( match route_method::<PluginId, Ctx>( socket, ctx, interface_path, function, data ) {
        Ok( val ) => Ok( Some( Box::new( val ))),
        Err( err ) => Err( Some( Box::new( err.into() ))),
    })
}

#[inline]
fn dispatch_of<PluginId, Ctx>(
    ctx: &mut StoreContextMut<Ctx>,
    plugin: &Mutex<PluginInstance<PluginId, Ctx>>,
    interface_path: &str,
    function: &Function,
    data: &[Val],
) -> Result<Val, DispatchError>
where
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext,
{

    let mut lock = plugin.lock().map_err(|_| DispatchError::LockRejected )?;
    let has_return = function.return_kind() != ReturnKind::Void ;
    let result = lock.dispatch( interface_path, function.name(), has_return, data )?;

    Ok( match function.return_kind() {
        ReturnKind::Void | ReturnKind::AssumeNoResources => result,
        ReturnKind::MayContainResources => wrap_resources( result, lock.id(), ctx )?,
    })
}

#[inline]
fn route_method<PluginId, Ctx>(
    socket: &LoadedSocket<PluginId, Ctx>,
    mut ctx: StoreContextMut<Ctx>,
    interface_path: &str,
    function: &Function,
    data: &[Val],
) -> Result<Val, DispatchError>
where
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
    Ctx: PluginContext,
{

    let handle = match data.first() {
        Some( Val::Resource( handle )) => Ok( handle ),
        _ => Err( DispatchError::InvalidArgumentList ),
    }?;

    let resource = ResourceWrapper::from_handle( *handle, &mut ctx )?;
    let plugin = socket.get( &resource.plugin_id )
        .map_err(|_| DispatchError::LockRejected )?
        .ok_or( DispatchError::InvalidArgumentList )?;

    let mut data = Vec::from( data );
    data[0] = Val::Resource( resource.resource_handle );

    dispatch_of( &mut ctx, plugin, interface_path, function, &data )

}

fn wrap_resources<T, Id>( val: Val, plugin_id: &Id, store: &mut StoreContextMut<T> ) -> Result<Val, DispatchError>
where
    T: PluginContext,
    Id: std::fmt::Debug + Clone + Send + Sync + 'static,
{
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
        Val::Record( entries ) => Val::Record( entries.into_iter()
            .map(|( key, value )| Ok::<_, DispatchError>(( key, wrap_resources( value, plugin_id, store )?)) )
            .collect::<Result<_,_>>()?
        ),
        Val::Tuple( list ) => Val::Tuple( list.into_iter().map(| item | wrap_resources( item, plugin_id, store )).collect::<Result<_,_>>()? ),
        Val::Variant( variant, Some( data_box )) => Val::Variant( variant, Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
        Val::Option( Some( data_box )) => Val::Option( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
        Val::Result( Ok( Some( data_box ))) => Val::Result( Ok( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
        Val::Result( Err( Some( data_box ))) => Val::Result( Err( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
        Val::Resource( handle ) => Val::Resource( ResourceWrapper::new( plugin_id.clone(), handle ).attach( store )? ),
        Val::Future( _ ) => return Err( DispatchError::UnsupportedType( "future".to_string() )),
        Val::Stream( _ ) => return Err( DispatchError::UnsupportedType( "stream".to_string() )),
        Val::ErrorContext( _ ) => return Err( DispatchError::UnsupportedType( "error-context".to_string() )),
    })
}
