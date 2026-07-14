use std::collections::HashMap ;
use std::sync::mpsc ;
use thiserror::Error ;
use wasmtime::component::{ Instance, Val };
use wasmtime::Store ;

use crate::{ Function, PluginContext, Remap, ReturnKind };
use crate::resource_wrapper::{ ResourceCreationError, ResourceReceiveError };

type CallLimiter<Ctx> = Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>;


/// An instantiated plugin with its store and instance, ready for dispatch.
///
/// Created by calling [`Plugin::instantiate`]( crate::Plugin::instantiate ),
/// [`Plugin::link`]( crate::Plugin::link ), [`Plugin::instantiate_async`]( crate::Plugin::instantiate_async ),
/// or [`Plugin::link_async`]( crate::Plugin::link_async ). Synchronous instances hold their Wasmtime
/// [`Store`] directly. Async instances keep their independent store on a serialized worker.
pub struct PluginInstance<Ctx: 'static> {
	runtime: PluginRuntime<Ctx>,
}

enum PluginRuntime<Ctx: 'static> {
	Sync( PluginState<Ctx> ),
	Async( mpsc::Sender<AsyncDispatch> ),
}

struct PluginState<Ctx: 'static> {
	store: Store<Ctx>,
	instance: Instance,
	interface_remaps: HashMap<String, Remap>,
	fuel_limiter: Option<CallLimiter<Ctx>>,
	epoch_limiter: Option<CallLimiter<Ctx>>,
}

struct AsyncDispatch {
	package_name: String,
	interface_name: String,
	function_name: String,
	function: Function,
	data: Vec<Val>,
	response: futures::channel::oneshot::Sender<Result<Val, DispatchError>>,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstance<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		let mut debug = f.debug_struct( "PluginInstance" );
		match &self.runtime {
			PluginRuntime::Sync( state ) => debug
				.field( "data", &state.store.data() )
				.field( "store", &state.store )
				.field( "interface_remaps", &state.interface_remaps )
				.field( "fuel_limiter", &state.fuel_limiter.as_ref().map(| _ | "<closure>" ))
				.field( "epoch_limiter", &state.epoch_limiter.as_ref().map(| _ | "<closure>" )),
			PluginRuntime::Async( _ ) => debug.field( "runtime", &"<async worker>" ),
		};
		debug.finish_non_exhaustive()
	}
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside a cardinality wrapper from
/// [`Binding::dispatch`]( crate::binding::Binding::dispatch )
/// when a function call fails at runtime.
#[derive( Error, Debug )]
pub enum DispatchError {
	/// Failed to acquire lock on plugin instance (another call is in progress).
	#[error( "Lock Rejected" )] LockRejected,
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
	/// This instance was asynchronously instantiated and must be dispatched asynchronously.
	#[error( "Async dispatch required" )] AsyncRequired,
	/// The worker owning an asynchronously instantiated plugin stopped unexpectedly.
	#[error( "Async plugin worker stopped" )] AsyncWorkerStopped,
	/// Failed to create a resource handle for cross-plugin transfer.
	#[error( "Resource Create Error: {0}" )] ResourceCreationError( #[from] ResourceCreationError ),
	/// Failed to receive a resource handle from another plugin.
	#[error( "Resource Receive Error: {0}" )] ResourceReceiveError( #[from] ResourceReceiveError ),
}

impl From<DispatchError> for Val {
	fn from( error: DispatchError ) -> Val { match error {
		DispatchError::LockRejected => Val::Variant( "lock-rejected".to_string(), None ),
		DispatchError::InvalidInterfacePath( package ) => Val::Variant( "invalid-interface-path".to_string(), Some( Box::new( Val::String( package )))),
		DispatchError::InvalidFunction( function ) => Val::Variant( "invalid-function".to_string(), Some( Box::new( Val::String( function )))),
		DispatchError::MissingResponse => Val::Variant( "missing-response".to_string(), None ),
		DispatchError::RuntimeException( exception ) => Val::Variant( "runtime-exception".to_string(), Some( Box::new( Val::String( exception.to_string() )))),
		DispatchError::InvalidArgumentList => Val::Variant( "invalid-argument-list".to_string(), None ),
		DispatchError::UnsupportedType( name ) => Val::Variant( "unsupported-type".to_string(), Some( Box::new( Val::String( name )))),
		DispatchError::AsyncRequired => Val::Variant( "async-required".to_string(), None ),
		DispatchError::AsyncWorkerStopped => Val::Variant( "async-worker-stopped".to_string(), None ),
		DispatchError::ResourceCreationError( err ) => err.into(),
		DispatchError::ResourceReceiveError( err ) => err.into(),
	}}
}

impl<Ctx: PluginContext + 'static> PluginInstance<Ctx> {
	pub(crate) fn new_sync(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
	) -> Self {
		Self { runtime: PluginRuntime::Sync( PluginState {
			store,
			instance,
			interface_remaps,
			fuel_limiter,
			epoch_limiter,
		})}
	}

	pub(crate) fn new_async(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
	) -> Result<Self, wasmtime::Error> {
		let ( sender, receiver ) = mpsc::channel::<AsyncDispatch>();
		let mut state = PluginState {
			store,
			instance,
			interface_remaps,
			fuel_limiter,
			epoch_limiter,
		};
		std::thread::Builder::new()
			.name( "wasm-link-plugin".to_string() )
			.spawn( move || while let Ok( request ) = receiver.recv() {
				let result = futures::executor::block_on( state.dispatch_async(
					&request.package_name,
					&request.interface_name,
					&request.function_name,
					&request.function,
					&request.data,
				));
				let _ = request.response.send( result );
			})
			.map_err(| error | wasmtime::Error::msg( format!( "failed to start async plugin worker: {error}" )))?;
		Ok( Self { runtime: PluginRuntime::Async( sender )})
	}

	pub(crate) fn dispatch(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		match &mut self.runtime {
			PluginRuntime::Sync( state ) => state.dispatch( package_name, interface_name, function_name, function, data ),
			PluginRuntime::Async( _ ) => Err( DispatchError::AsyncRequired ),
		}
	}

	pub(crate) async fn dispatch_async(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		let PluginRuntime::Async( sender ) = &self.runtime else {
			return Err( DispatchError::AsyncRequired );
		};
		let ( response, result ) = futures::channel::oneshot::channel();
		sender.send( AsyncDispatch {
			package_name: package_name.to_string(),
			interface_name: interface_name.to_string(),
			function_name: function_name.to_string(),
			function: function.clone(),
			data: data.to_vec(),
			response,
		}).map_err(| _ | DispatchError::AsyncWorkerStopped )?;
		result.await.map_err(| _ | DispatchError::AsyncWorkerStopped )?
	}

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
		self.finish_call( function, buffer, call_result )
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
		self.finish_call( function, buffer, call_result )
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
		&mut self,
		function: &Function,
		mut buffer: Vec<Val>,
		call_result: Result<(), wasmtime::Error>,
	) -> Result<Val, DispatchError> {
		if self.fuel_limiter.is_some() { let _ = self.store.set_fuel( 0 ); }
		call_result.map_err( DispatchError::RuntimeException )?;
		let result = match function.return_kind() != ReturnKind::Void {
			true => buffer.pop().ok_or( DispatchError::MissingResponse )?,
			false => Self::VOID_RETURN_VAL,
		};
		ensure_supported_value( &result )?;
		Ok( result )
	}

	fn resolve_export( &self, package_name: &str, interface_name: &str, function_name: &str ) -> (String, String) {
		match self.interface_remaps.get( interface_name ) {
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
