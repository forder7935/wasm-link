use thiserror::Error ;
use wasmtime::component::{ Instance, Val };
use wasmtime::Store ;

use crate::PluginContext ;
use crate::resource_wrapper::{ ResourceCreationError, ResourceReceiveError };



pub struct PluginInstance<Ctx: 'static> {
    pub(crate) store: Store<Ctx>,
    pub(crate) instance: Instance,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstance<Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct( "PluginInstance" )
            .field( "data", &self.store.data() )
            .field( "store", &self.store )
            .finish_non_exhaustive()
    }
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside [`Socket`]( crate::socket::Socket ) From
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
        function: &str,
        returns: bool,
        data: &[Val],
    ) -> Result<Val, DispatchError> {

        let mut buffer = match returns {
            true => vec![ Self::PLACEHOLDER_VAL ],
            false => Vec::with_capacity( 0 ),
        };

        let interface_index = self.instance
            .get_export_index( &mut self.store, None, interface_path )
            .ok_or( DispatchError::InvalidInterfacePath( interface_path.to_string() ))?;
        let func_index = self.instance
            .get_export_index( &mut self.store, Some( &interface_index ), function )
            .ok_or( DispatchError::InvalidFunction( format!( "{}:{}", interface_path, function )))?;
        let func = self.instance
            .get_func( &mut self.store, func_index )
            .ok_or( DispatchError::InvalidFunction( format!( "{}:{}", interface_path, function )))?;
        func
            .call( &mut self.store, data, &mut buffer )
            .map_err( DispatchError::RuntimeException )?;
        let _ = func.post_return( &mut self.store );

        Ok( match returns {
            true => buffer.pop().ok_or( DispatchError::MissingResponse )?,
            false => Self::PLACEHOLDER_VAL,
        })

    }
}
