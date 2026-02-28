use std::sync::Mutex ;
use wasmtime::StoreContextMut ;
use wasmtime::component::Val ;

use crate::{ Binding, Function, FunctionKind, ReturnKind, PluginContext, DispatchError };
use crate::cardinality::Cardinality ;
use crate::plugin_instance::PluginInstance ;
use super::resource_wrapper::ResourceWrapper ;



/// Dispatches a non-method function call to all plugins
pub(crate) fn dispatch_all<PluginId, Ctx, Plugins>(
	binding: &Binding<PluginId, Ctx, Plugins>,
	mut ctx: StoreContextMut<Ctx>,
	interface_path: &str,
	function_name: &str,
	function: &Function,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext,
	Plugins: Cardinality<PluginId, PluginInstance<Ctx>>,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Send + Sync,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>,
	<<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>> as Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>>::Rebind<Val>: Into<Val>,
{
	debug_assert_eq!( function.kind(), FunctionKind::Freestanding );
	binding.plugins().map(| plugin_id, plugin | Val::Result(
		match dispatch_of(
			&mut ctx,
			plugin_id.clone(),
			plugin,
			interface_path,
			function_name,
			function,
			data,
		) {
			Ok( val ) => Ok( Some( Box::new( val ))),
			Err( err ) => Err( Some( Box::new( err.into() ))),
		}
	)).into()
}

/// Dispatches a method function call, routing to the correct plugin.
pub(crate) fn dispatch_method<PluginId, Ctx, Plugins>(
	binding: &Binding<PluginId, Ctx, Plugins>,
	ctx: StoreContextMut<Ctx>,
	interface_path: &str,
	function_name: &str,
	function: &Function,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Plugins: Cardinality<PluginId, PluginInstance<Ctx>>,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Send + Sync,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>,
{
	debug_assert_eq!( function.kind(), FunctionKind::Method );
	Val::Result( match route_method(
		binding,
		ctx,
		interface_path,
		function_name,
		function,
		data,
	) {
		Ok( val ) => Ok( Some( Box::new( val ))),
		Err( err ) => Err( Some( Box::new( err.into() ))),
	})
}

#[inline]
fn dispatch_of<PluginId, Ctx>(
	ctx: &mut StoreContextMut<Ctx>,
	plugin_id: PluginId,
	plugin: &Mutex<PluginInstance<Ctx>>,
	interface_path: &str,
	function_name: &str,
	function: &Function,
	data: &[Val],
) -> Result<Val, DispatchError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
{

	let mut lock = plugin.lock().map_err(|_| DispatchError::LockRejected )?;
	let result = lock.dispatch( interface_path, function_name, function, data )?;

	Ok( match function.return_kind() {
		ReturnKind::Void | ReturnKind::AssumeNoResources => result,
		ReturnKind::MayContainResources => wrap_resources( result, plugin_id, ctx )?,
	})
}

#[inline]
fn route_method<PluginId, Ctx, Plugins>(
	binding: &Binding<PluginId, Ctx, Plugins>,
	mut ctx: StoreContextMut<Ctx>,
	interface_path: &str,
	function_name: &str,
	function: &Function,
	data: &[Val],
) -> Result<Val, DispatchError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Plugins: Cardinality<PluginId, PluginInstance<Ctx>>,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Send + Sync,
	<Plugins as Cardinality<PluginId, PluginInstance<Ctx>>>::Rebind<Mutex<PluginInstance<Ctx>>>: Cardinality<PluginId, Mutex<PluginInstance<Ctx>>>,
{

	let handle = match data.first() {
		Some( Val::Resource( handle )) => Ok( handle ),
		_ => Err( DispatchError::InvalidArgumentList ),
	}?;

	let resource = ResourceWrapper::<PluginId>::from_handle( *handle, &mut ctx )?;
	let plugin = binding.plugins().get( &resource.plugin_id ).ok_or( DispatchError::InvalidArgumentList )?;
	let plugin_id = resource.plugin_id.clone();

	let mut data = Vec::from( data );
	data[0] = Val::Resource( resource.resource_handle );

	dispatch_of(
		&mut ctx,
		plugin_id,
		plugin,
		interface_path,
		function_name,
		function,
		&data,
	)

}

fn wrap_resources<T, Id>( val: Val, plugin_id: Id, store: &mut StoreContextMut<T> ) -> Result<Val, DispatchError>
where
	T: PluginContext,
	Id: Clone + Send + Sync + 'static,
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
		Val::List( list ) => Val::List( list.into_iter().map(| item | wrap_resources( item, plugin_id.clone(), store )).collect::<Result<_,_>>()? ),
		Val::Record( entries ) => Val::Record( entries.into_iter()
			.map(|( key, value )| Ok::<_, DispatchError>(( key, wrap_resources( value, plugin_id.clone(), store )?)) )
			.collect::<Result<_,_>>()?
		),
		Val::Tuple( list ) => Val::Tuple( list.into_iter().map(| item | wrap_resources( item, plugin_id.clone(), store )).collect::<Result<_,_>>()? ),
		Val::Variant( variant, Some( data_box )) => Val::Variant( variant, Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
		Val::Option( Some( data_box )) => Val::Option( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? ))),
		Val::Result( Ok( Some( data_box ))) => Val::Result( Ok( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
		Val::Result( Err( Some( data_box ))) => Val::Result( Err( Some( Box::new( wrap_resources( *data_box, plugin_id, store )? )))),
		Val::Resource( handle ) => Val::Resource( ResourceWrapper::new( plugin_id, handle ).attach( store )? ),
		Val::Future( _ ) => return Err( DispatchError::UnsupportedType( "future".to_string() )),
		Val::Stream( _ ) => return Err( DispatchError::UnsupportedType( "stream".to_string() )),
		Val::ErrorContext( _ ) => return Err( DispatchError::UnsupportedType( "error-context".to_string() )),
	})
}
