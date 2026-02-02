use thiserror::Error ;
use wasmtime::component::{ Component, Instance, Val };
use wasmtime::Store ;

use crate::plugin::PluginData ;
use crate::interface::InterfaceData ;
use crate::loading::{ ResourceCreationError, ResourceReceiveError };



pub struct PluginInstance<P: PluginData + 'static> {
    pub(crate) id: P::Id,
    pub(crate) _component: Component,
    pub(crate) store: Store<P>,
    pub(crate) instance: Instance,
}

impl<P: PluginData + std::fmt::Debug> std::fmt::Debug for PluginInstance<P> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct( "Plugin Instance" )
            .field( "id", &self.id )
            .field( "data", &self.store.data() )
            .field( "store", &self.store )
            .finish_non_exhaustive()
    }
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside the [`Socket`] from [`PluginTreeHead::dispatch`] when a
/// function call fails at runtime.
///
/// [`Socket`]: crate::Socket
/// [`PluginTreeHead::dispatch`]: crate::PluginTreeHead::dispatch
#[derive( Error, Debug )]
pub enum DispatchError<I: InterfaceData> {
    /// Failed to acquire lock on plugin instance (another call is in progress).
    #[error( "Deadlock" )] Deadlock,
    /// Failed to parse interface metadata during dispatch.
    #[error( "Interface Error: {0}")] WitParserError( I::Error ),
    /// The specified interface path doesn't match any known interface.
    #[error( "Invalid Interface: {0}" )] InvalidInterface( String ),
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

impl<I: InterfaceData> From<DispatchError<I>> for Val {
    fn from( error: DispatchError<I> ) -> Val { match error {
        DispatchError::Deadlock => Val::Variant( "deadlock".to_string(), None ),
        DispatchError::WitParserError( err ) => Val::Variant( "wit-parser-error".to_string(), Some( Box::new( Val::String( err.to_string() )))),
        DispatchError::InvalidInterface( package ) => Val::Variant( "invalid-interface".to_string(), Some( Box::new( Val::String( package )))),
        DispatchError::InvalidFunction( function ) => Val::Variant( "invalid-function".to_string(), Some( Box::new( Val::String( function )))),
        DispatchError::MissingResponse => Val::Variant( "missing-response".to_string(), None ),
        DispatchError::RuntimeException( exception ) => Val::Variant( "runtime-exception".to_string(), Some( Box::new( Val::String( exception.to_string() )))),
        DispatchError::InvalidArgumentList => Val::Variant( "invalid-argument-list".to_string(), None ),
		DispatchError::UnsupportedType( name ) => Val::Variant( "unsupported-type".to_string(), Some( Box::new( Val::String( name )))),
        DispatchError::ResourceCreationError( err ) => err.into(),
        DispatchError::ResourceReceiveError( err ) => err.into(),
    }}
}

impl<P: PluginData> PluginInstance<P> {

    pub fn id( &self ) -> &P::Id { &self.id }

    const PLACEHOLDER_VAL: Val = Val::Tuple( vec![] );

    pub(crate) fn dispatch<I: InterfaceData>(
        &mut self,
        interface_path: &str,
        function: &str,
        returns: bool,
        data: &[Val],
    ) -> Result<Val, DispatchError<I>> {

        let mut buffer = match returns {
            true => vec![ Self::PLACEHOLDER_VAL ],
            false => Vec::with_capacity( 0 ),
        };

        let interface_index = self.instance
            .get_export_index( &mut self.store, None, interface_path )
            .ok_or( DispatchError::InvalidInterface( interface_path.to_string() ))?;
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
