use std::collections::{ HashMap, HashSet };
use std::cell::RefCell ;
use std::future::Future ;
use std::pin::Pin ;
use std::sync::Arc ;
use futures::future::BoxFuture ;
use futures::lock::Mutex ;
use futures::stream::{ FuturesUnordered, Stream };
use futures::task::AtomicWaker ;
use thiserror::Error ;
use wasmtime::component::{ Instance, Val };
use wasmtime::Store ;

use crate::{ Function, PluginContext, Remap, ReturnKind };
use crate::resource_wrapper::{ ResourceCreationError, ResourceReceiveError };

type CallLimiter<Ctx> = Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>;

thread_local! {
	static CURRENT_DRIVER: RefCell<Option<Arc<DispatchDriver>>> = const { RefCell::new( None )};
}

pub(crate) struct DispatchDriver {
	incoming: std::sync::Mutex<Vec<BoxFuture<'static, ()>>>,
	waker: AtomicWaker,
}

struct DriverScope {
	previous: Option<Arc<DispatchDriver>>,
}

impl DispatchDriver {
	pub(crate) fn new() -> Arc<Self> {
		Arc::new( Self {
			incoming: std::sync::Mutex::new( Vec::new() ),
			waker: AtomicWaker::new(),
		})
	}

	pub(crate) fn current() -> Option<Arc<Self>> {
		CURRENT_DRIVER.with(| driver | driver.borrow().clone() )
	}

	pub(crate) fn spawn( &self, future: BoxFuture<'static, ()> ) {
		lock_unpoisoned( &self.incoming ).push( future );
		self.waker.wake();
	}

	pub(crate) async fn run<F>( self: &Arc<Self>, future: F ) -> F::Output
	where
		F: Future,
	{
		let mut future = std::pin::pin!( future );
		let mut tasks = FuturesUnordered::<BoxFuture<'static, ()>>::new();
		futures::future::poll_fn(| cx | {
			self.waker.register( cx.waker() );
			if let std::task::Poll::Ready( output ) = self.with_driver(|| future.as_mut().poll( cx )) {
				return std::task::Poll::Ready( output );
			}

			loop {
				tasks.extend( lock_unpoisoned( &self.incoming ).drain( .. ));
				if !matches!(
					self.with_driver(|| Pin::new( &mut tasks ).poll_next( cx )),
					std::task::Poll::Ready( Some(()))
				) && lock_unpoisoned( &self.incoming ).is_empty() {
					return std::task::Poll::Pending;
				}
			}
		}).await
	}

	fn with_driver<R>(
		self: &Arc<Self>,
		poll: impl FnOnce() -> R,
	) -> R {
		CURRENT_DRIVER.with(| driver | {
			let previous = driver.replace( Some( Arc::clone( self )));
			let _scope = DriverScope { previous };
			poll()
		})
	}
}

impl Drop for DriverScope {
	fn drop( &mut self ) {
		CURRENT_DRIVER.with(| driver | {
			driver.replace( self.previous.take() );
		});
	}
}

pub(crate) trait AsyncDispatchInstance<Ctx>:
	ExportEffectInstance + Clone + Send + Sync + 'static
where
	Ctx: PluginContext + 'static,
{
	#[allow( clippy::too_many_arguments )]
	fn dispatch_for_async<'a>(
		&'a self,
		driver: &'a Arc<DispatchDriver>,
		package_name: &'a str,
		interface_name: &'a str,
		function_name: &'a str,
		function: &'a Function,
		data: &'a [Val],
	) -> BoxFuture<'a, Result<Val, DispatchError>>;
}

pub(crate) trait ExportEffectInstance {
	fn export_is_async(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> bool;
}


/// A synchronously instantiated plugin, ready for synchronous dispatch.
///
/// Created by calling [`Plugin::instantiate`]( crate::Plugin::instantiate ),
/// or [`Plugin::link`]( crate::Plugin::link ).
pub struct PluginInstanceSync<Ctx: 'static> {
	state: Arc<std::sync::Mutex<PluginState<Ctx>>>,
	metadata: Arc<PluginMetadata>,
}

impl<Ctx: 'static> Clone for PluginInstanceSync<Ctx> {
	fn clone( &self ) -> Self {
		Self {
			state: Arc::clone( &self.state ),
			metadata: Arc::clone( &self.metadata ),
		}
	}
}

/// An asynchronously instantiated plugin, ready for asynchronous dispatch.
///
/// Created by calling [`Plugin::instantiate_async`]( crate::Plugin::instantiate_async )
/// or [`Plugin::link_async`]( crate::Plugin::link_async ). Each destination services
/// one call at a time. Dispatch futures cooperatively serialize the plugin's
/// Wasmtime [`Store`].
pub struct PluginInstanceAsync<Ctx: 'static> {
	state: Arc<Mutex<PluginState<Ctx>>>,
	metadata: Arc<PluginMetadata>,
}

impl<Ctx: 'static> Clone for PluginInstanceAsync<Ctx> {
	fn clone( &self ) -> Self {
		Self {
			state: Arc::clone( &self.state ),
			metadata: Arc::clone( &self.metadata ),
		}
	}
}

struct PluginState<Ctx: 'static> {
	store: Store<Ctx>,
	instance: Instance,
	metadata: Arc<PluginMetadata>,
	fuel_limiter: Option<CallLimiter<Ctx>>,
	epoch_limiter: Option<CallLimiter<Ctx>>,
}

struct PluginMetadata {
	interface_remaps: HashMap<String, Remap>,
	async_exports: HashSet<(String, String)>,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstanceSync<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		let state = lock_unpoisoned( &self.state );
		f.debug_struct( "PluginInstanceSync" )
			.field( "data", &state.store.data() )
			.field( "store", &state.store )
			.field( "interface_remaps", &state.metadata.interface_remaps )
			.field( "fuel_limiter", &state.fuel_limiter.as_ref().map(| _ | "<closure>" ))
			.field( "epoch_limiter", &state.epoch_limiter.as_ref().map(| _ | "<closure>" ))
			.finish_non_exhaustive()
	}
}

impl<Ctx: 'static> std::fmt::Debug for PluginInstanceAsync<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		f.debug_struct( "PluginInstanceAsync" )
			.field( "state", &"<serialized store>" )
			.finish_non_exhaustive()
	}
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside a cardinality wrapper from
/// [`Binding::dispatch`]( crate::binding::Binding::dispatch )
/// when a function call fails at runtime.
#[derive( Error, Debug )]
pub enum DispatchError {
	/// The specified interface path doesn't match any known interface.
	#[error( "Invalid Interface Path: {0}" )] InvalidInterfacePath( String ),
	/// The specified function doesn't exist on the interface.
	#[error( "Invalid Function: {0}" )] InvalidFunction( String ),
	/// Function was expected to return a value but didn't.
	#[error( "Missing Response" )] MissingResponse,
	/// The WASM function threw an exception during execution.
	#[error( "Runtime Exception" )] RuntimeException( wasmtime::Error ),
	/// The provided arguments don't match the function signature.
	#[error( "Invalid Argument List" )] InvalidArgumentList,
	/// Async types (`Future`, `Stream`, `ErrorContext`) are not yet supported for cross-plugin transfer.
	#[error( "Unsupported type: {0}" )] UnsupportedType( String ),
	/// Failed to create a resource handle for cross-plugin transfer.
	#[error( "Resource Create Error: {0}" )] ResourceCreationError( #[from] ResourceCreationError ),
	/// Failed to receive a resource handle from another plugin.
	#[error( "Resource Receive Error: {0}" )] ResourceReceiveError( #[from] ResourceReceiveError ),
}

impl From<DispatchError> for Val {
	fn from( error: DispatchError ) -> Val { match error {
		DispatchError::InvalidInterfacePath( package ) => Val::Variant( "invalid-interface-path".to_string(), Some( Box::new( Val::String( package )))),
		DispatchError::InvalidFunction( function ) => Val::Variant( "invalid-function".to_string(), Some( Box::new( Val::String( function )))),
		DispatchError::MissingResponse => Val::Variant( "missing-response".to_string(), None ),
		DispatchError::RuntimeException( exception ) => Val::Variant( "runtime-exception".to_string(), Some( Box::new( Val::String( exception.to_string() )))),
		DispatchError::InvalidArgumentList => Val::Variant( "invalid-argument-list".to_string(), None ),
		DispatchError::UnsupportedType( name ) => Val::Variant( "unsupported-type".to_string(), Some( Box::new( Val::String( name )))),
		DispatchError::ResourceCreationError( err ) => err.into(),
		DispatchError::ResourceReceiveError( err ) => err.into(),
	}}
}

impl<Ctx: PluginContext + 'static> PluginInstanceSync<Ctx> {
	pub(crate) fn new_sync(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
		async_exports: HashSet<(String, String)>,
	) -> Self {
		let metadata = Arc::new( PluginMetadata { interface_remaps, async_exports });
		Self {
			state: Arc::new( std::sync::Mutex::new( PluginState {
				store,
				instance,
				metadata: Arc::clone( &metadata ),
				fuel_limiter,
				epoch_limiter,
			})),
			metadata,
		}
	}

	pub(crate) fn dispatch_from(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		lock_unpoisoned( &self.state )
			.dispatch( package_name, interface_name, function_name, function, data )
	}
}

impl<Ctx: 'static> ExportEffectInstance for PluginInstanceSync<Ctx> {
	fn export_is_async(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> bool {
		let export = resolve_export(
			&self.metadata.interface_remaps,
			package_name,
			interface_name,
			function_name,
		);
		self.metadata.async_exports.contains( &export )
	}
}

impl<Ctx> AsyncDispatchInstance<Ctx> for PluginInstanceSync<Ctx>
where
	Ctx: PluginContext + 'static,
{
	fn dispatch_for_async<'a>(
		&'a self,
		driver: &'a Arc<DispatchDriver>,
		package_name: &'a str,
		interface_name: &'a str,
		function_name: &'a str,
		function: &'a Function,
		data: &'a [Val],
	) -> BoxFuture<'a, Result<Val, DispatchError>> {
		let instance = self.clone();
		let driver = Arc::clone( driver );
		let package_name = package_name.to_string();
		let interface_name = interface_name.to_string();
		let function_name = function_name.to_string();
		let function = function.clone();
		let data = data.to_vec();
		Box::pin( async move {
			let ( response, result ) = futures::channel::oneshot::channel();
			driver.spawn( Box::pin( async move {
				let result = instance.dispatch_from(
					&package_name, &interface_name, &function_name, &function, &data,
				);
				let _ = response.send( result );
			}));
			result.await.map_err(| _ | DispatchError::MissingResponse )?
		})
	}
}

impl<Ctx> PluginInstanceAsync<Ctx>
where
	Ctx: PluginContext + 'static,
{
	pub(crate) fn new(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
		async_exports: HashSet<(String, String)>,
	) -> Self {
		let metadata = Arc::new( PluginMetadata { interface_remaps, async_exports });
		Self {
			state: Arc::new( Mutex::new( PluginState {
				store,
				instance,
				metadata: Arc::clone( &metadata ),
				fuel_limiter,
				epoch_limiter,
			})),
			metadata,
		}
	}

	pub(crate) async fn dispatch_async_from(
		&self,
		driver: &Arc<DispatchDriver>,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let state = Arc::clone( &self.state );
		let driver = Arc::clone( driver );
		let package_name = package_name.to_string();
		let interface_name = interface_name.to_string();
		let function_name = function_name.to_string();
		let function = function.clone();
		let data = data.to_vec();
		let ( response, result ) = futures::channel::oneshot::channel();
		driver.spawn( Box::pin( async move {
			let result = state.lock().await.dispatch_async(
				&package_name, &interface_name, &function_name, &function, &data,
			).await;
			let _ = response.send( result );
		}));
		result.await.map_err(| _ | DispatchError::MissingResponse )?
	}
}

impl<Ctx: 'static> ExportEffectInstance for PluginInstanceAsync<Ctx> {
	fn export_is_async(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> bool {
		let export = resolve_export(
			&self.metadata.interface_remaps,
			package_name,
			interface_name,
			function_name,
		);
		self.metadata.async_exports.contains( &export )
	}
}

impl<Ctx> AsyncDispatchInstance<Ctx> for PluginInstanceAsync<Ctx>
where
	Ctx: PluginContext + 'static,
{
	fn dispatch_for_async<'a>(
		&'a self,
		driver: &'a Arc<DispatchDriver>,
		package_name: &'a str,
		interface_name: &'a str,
		function_name: &'a str,
		function: &'a Function,
		data: &'a [Val],
	) -> BoxFuture<'a, Result<Val, DispatchError>> {
		Box::pin( self.dispatch_async_from(
			driver,
			package_name,
			interface_name,
			function_name,
			function,
			data,
		))
	}
}

fn lock_unpoisoned<T>( mutex: &std::sync::Mutex<T> ) -> std::sync::MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else( std::sync::PoisonError::into_inner )
}

impl<Ctx: PluginContext + 'static> PluginState<Ctx> {
	const PLACEHOLDER_VAL: Val = Val::Option( None );
	const VOID_RETURN_VAL: Val = Val::Option( None );

	fn dispatch(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let mut buffer = self.prepare_call( package_name, interface_name, function_name, function )?;
		let ( exported_interface_path, exported_function_name ) = self.resolve_export( package_name, interface_name, function_name );
		let func = self.function( &exported_interface_path, &exported_function_name )?;
		let call_result = func.call( &mut self.store, data, &mut buffer );
		Self::finish_call( function, buffer, call_result )
	}

	async fn dispatch_async(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let mut buffer = self.prepare_call( package_name, interface_name, function_name, function )?;
		let ( exported_interface_path, exported_function_name ) = self.resolve_export( package_name, interface_name, function_name );
		let func = self.function( &exported_interface_path, &exported_function_name )?;
		let call_result = func.call_async( &mut self.store, data, &mut buffer ).await;
		Self::finish_call( function, buffer, call_result )
	}

	fn prepare_call(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
	) -> Result<Vec<Val>, DispatchError> {
		let canonical_interface_path = format!( "{}/{}", package_name, interface_name );
		if let Some( mut limiter ) = self.fuel_limiter.take() {
			let fuel = limiter( &mut self.store, &canonical_interface_path, function_name, function );
			self.fuel_limiter = Some( limiter );
			self.store.set_fuel( fuel ).map_err( DispatchError::RuntimeException )?;
		}
		if let Some( mut limiter ) = self.epoch_limiter.take() {
			let ticks = limiter( &mut self.store, &canonical_interface_path, function_name, function );
			self.epoch_limiter = Some( limiter );
			self.store.set_epoch_deadline( ticks );
		}
		Ok( match function.return_kind() != ReturnKind::Void {
			true => vec![ Self::PLACEHOLDER_VAL ],
			false => Vec::with_capacity( 0 ),
		})
	}

	fn function( &mut self, interface_path: &str, function_name: &str ) -> Result<wasmtime::component::Func, DispatchError> {
		let interface_index = self.instance
			.get_export_index( &mut self.store, None, interface_path )
			.ok_or_else(|| DispatchError::InvalidInterfacePath( interface_path.to_string() ))?;
		let func_index = self.instance
			.get_export_index( &mut self.store, Some( &interface_index ), function_name )
			.ok_or_else(|| DispatchError::InvalidFunction( format!( "{interface_path}:{function_name}" )))?;
		self.instance
			.get_func( &mut self.store, func_index )
			.ok_or_else(|| DispatchError::InvalidFunction( format!( "{interface_path}:{function_name}" )))
	}

	fn finish_call(
		function: &Function,
		mut buffer: Vec<Val>,
		call_result: Result<(), wasmtime::Error>,
	) -> Result<Val, DispatchError> {
		call_result.map_err( DispatchError::RuntimeException )?;
		let result = match function.return_kind() != ReturnKind::Void {
			true => buffer.pop().ok_or( DispatchError::MissingResponse )?,
			false => Self::VOID_RETURN_VAL,
		};
		ensure_supported_value( &result )?;
		Ok( result )
	}

	fn resolve_export( &self, package_name: &str, interface_name: &str, function_name: &str ) -> (String, String) {
		resolve_export( &self.metadata.interface_remaps, package_name, interface_name, function_name )
	}

}

fn resolve_export(
	interface_remaps: &HashMap<String, Remap>,
	package_name: &str,
	interface_name: &str,
	function_name: &str,
) -> (String, String) {
	match interface_remaps.get( interface_name ) {
		Some( remap ) => (
			format!( "{}/{}", package_name, remap.interface_name( interface_name )),
			remap.item_name( function_name ).to_string(),
		),
		None => (
			format!( "{}/{}", package_name, interface_name ),
			function_name.to_string(),
		),
	}
}

fn ensure_supported_values( values: &[Val] ) -> Result<(), DispatchError> {
	values.iter().try_for_each( ensure_supported_value )
}

fn ensure_supported_value( value: &Val ) -> Result<(), DispatchError> {
	match value {
		Val::List( values ) | Val::Tuple( values ) => ensure_supported_values( values ),
		Val::Map( values ) => values.iter().try_for_each(|( key, value )| {
			ensure_supported_value( key )?;
			ensure_supported_value( value )
		}),
		Val::Record( values ) => values.iter().try_for_each(|( _, value )| ensure_supported_value( value )),
		Val::Variant( _, Some( value ))
		| Val::Option( Some( value ))
		| Val::Result( Ok( Some( value )))
		| Val::Result( Err( Some( value ))) => ensure_supported_value( value ),
		Val::Future( _ ) => Err( DispatchError::UnsupportedType( "future".to_string() )),
		Val::Stream( _ ) => Err( DispatchError::UnsupportedType( "stream".to_string() )),
		Val::ErrorContext( _ ) => Err( DispatchError::UnsupportedType( "error-context".to_string() )),
		_ => Ok(()),
	}
}

#[cfg(test)] mod tests { include!( "plugin_instance_tests.rs" ); }
