use thiserror::Error ;
use wasmtime::component::{ Instance, Val };
use wasmtime::Store ;

use crate::{ Function, PluginContext, ReturnKind };
use crate::resource_wrapper::{ ResourceCreationError, ResourceReceiveError };



/// An instantiated plugin with its store and instance, ready for dispatch.
///
/// Created by calling [`Plugin::instantiate`]( crate::Plugin::instantiate ) or
/// [`Plugin::link`]( crate::Plugin::link ). The plugin holds its wasmtime [`Store`]
/// and can execute function calls.
pub struct PluginInstance<Ctx: 'static> {
	pub(crate) store: Store<Ctx>,
	pub(crate) instance: Instance,
	#[allow( clippy::type_complexity )]
	pub(crate) fuel_limiter: Option<Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>>,
	#[allow( clippy::type_complexity )]
	pub(crate) epoch_limiter: Option<Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>>,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstance<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		f.debug_struct( "PluginInstance" )
			.field( "data", &self.store.data() )
			.field( "store", &self.store )
			.field( "fuel_limiter", &self.fuel_limiter.as_ref().map(| _ | "<closure>" ))
			.field( "epoch_limiter", &self.epoch_limiter.as_ref().map(| _ | "<closure>" ))
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
		DispatchError::ResourceCreationError( err ) => err.into(),
		DispatchError::ResourceReceiveError( err ) => err.into(),
	}}
}

impl<Ctx: PluginContext + 'static> PluginInstance<Ctx> {

	const PLACEHOLDER_VAL: Val = Val::Tuple( vec![] );

	pub(crate) fn dispatch(
		&mut self,
		interface_path: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {

		let mut buffer = match function.return_kind() != ReturnKind::Void {
			true => vec![ Self::PLACEHOLDER_VAL ],
			false => Vec::with_capacity( 0 ),
		};

		let fuel_was_set = if let Some( mut limiter ) = self.fuel_limiter.take() {
			let fuel = limiter( &mut self.store, interface_path, function_name, function );
			self.fuel_limiter = Some( limiter );
			self.store.set_fuel( fuel ).map_err( DispatchError::RuntimeException )?;
			true
		} else { false };

		if let Some( mut limiter ) = self.epoch_limiter.take() {
			let ticks = limiter( &mut self.store, interface_path, function_name, function );
			self.epoch_limiter = Some( limiter );
			self.store.set_epoch_deadline( ticks );
		}

		let interface_index = self.instance
			.get_export_index( &mut self.store, None, interface_path )
			.ok_or( DispatchError::InvalidInterfacePath( interface_path.to_string() ))?;
		let func_index = self.instance
			.get_export_index( &mut self.store, Some( &interface_index ), function_name )
			.ok_or( DispatchError::InvalidFunction( format!( "{}:{}", interface_path, function_name )))?;
		let func = self.instance
			.get_func( &mut self.store, func_index )
			.ok_or( DispatchError::InvalidFunction( format!( "{}:{}", interface_path, function_name )))?;
		let call_result = func.call( &mut self.store, data, &mut buffer );

		// Reset fuel to 0 after call to prevent leakage to subsequent calls
		if fuel_was_set { let _ = self.store.set_fuel( 0 ); }

		call_result.map_err( DispatchError::RuntimeException )?;
		let _ = func.post_return( &mut self.store );

		Ok( match function.return_kind() != ReturnKind::Void {
			true => buffer.pop().ok_or( DispatchError::MissingResponse )?,
			false => Self::PLACEHOLDER_VAL,
		})

	}

}
