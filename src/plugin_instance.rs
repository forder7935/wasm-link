use std::collections::HashMap ;
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
    pub(crate) fuel_multiplier: Option<f64>,
    pub(crate) epoch_deadline_multiplier: Option<f64>,
    /// Nested map: interface -> function -> fuel
    pub(crate) fuel_overrides: HashMap<String, HashMap<String, u64>>,
    /// Nested map: interface -> function -> epoch deadline
    pub(crate) epoch_deadline_overrides: HashMap<String, HashMap<String, u64>>,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstance<Ctx> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct( "PluginInstance" )
            .field( "data", &self.store.data() )
            .field( "store", &self.store )
            .field( "fuel_multiplier", &self.fuel_multiplier )
            .field( "epoch_deadline_multiplier", &self.epoch_deadline_multiplier )
            .field( "fuel_overrides", &self.fuel_overrides )
            .field( "epoch_deadline_overrides", &self.epoch_deadline_overrides )
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
        function_name: &str,
        function: &Function,
        default_fuel: Option<u64>,
        default_epoch: Option<u64>,
        data: &[Val],
    ) -> Result<Val, DispatchError> {

        let mut buffer = match function.return_kind() != ReturnKind::Void {
            true => vec![ Self::PLACEHOLDER_VAL ],
            false => Vec::with_capacity( 0 ),
        };

        // Resolve and apply fuel/epoch limits
        let fuel_was_set = if let Some( fuel ) = self.resolve_fuel( interface_path, function_name, function, default_fuel ) {
            self.store.set_fuel( fuel ).map_err( DispatchError::RuntimeException )?;
            true
        } else { false };
        if let Some( ticks ) = self.resolve_epoch_deadline( interface_path, function_name, function, default_epoch ) {
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

    /// Resolves the fuel limit for a function call.
    ///
    /// Precedence: plugin override > (multiplier * function fuel) > (multiplier * binding default)
    fn resolve_fuel( &self, interface: &str, function_name: &str, function: &Function, default_fuel: Option<u64> ) -> Option<u64> {
        self.fuel_overrides.get( interface )
            .and_then(| fns | fns.get( function_name ))
            .copied()
            .or_else(|| {
                let base = function.fuel().or( default_fuel )?;
                Some( match self.fuel_multiplier {
                    Some( multiplier ) => scale_u64( base, multiplier ),
                    None => base,
                })
            })
    }

    /// Resolves the epoch deadline for a function call.
    ///
    /// Precedence: plugin override > (multiplier * function epoch) > (multiplier * binding default)
    fn resolve_epoch_deadline( &self, interface: &str, function_name: &str, function: &Function, default_ticks: Option<u64> ) -> Option<u64> {
        self.epoch_deadline_overrides.get( interface )
            .and_then(| fns | fns.get( function_name ))
            .copied()
            .or_else(|| {
                let base = function.epoch_deadline().or( default_ticks )?;
                Some( match self.epoch_deadline_multiplier {
                    Some( multiplier ) => scale_u64( base, multiplier ),
                    None => base,
                })
            })
    }

}

/// Scales a u64 by a f64 multiplier.
///
/// # Quirks
///
/// - **NaN, zero, or negative multipliers** → returns 0
/// - **Integer multipliers** (1.0, 2.0, etc.) → uses exact integer multiplication
/// - **Non-integer multipliers with base > 2^53** → base is truncated to fit f64's
///   52-bit mantissa before multiplication. Least significant bits are lost.
/// - **Overflow** → clamps to `u64::MAX`
#[allow( clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss )]
fn scale_u64( base: u64, multiplier: f64 ) -> u64 {

    if multiplier.is_nan() || multiplier <= 0.0 || base == 0 {
        return 0;
    }

    // Integer multipliers use exact integer arithmetic (no f64 conversion of base)
    if multiplier.fract() == 0.0 {
        if multiplier > u64::MAX as f64 {
            return u64::MAX;
        }
        return base.saturating_mul( multiplier as u64 );
    }

    // Non-integer multipliers: base is converted to f64 (truncates if > 2^53)
    let result = ( base as f64 ) * multiplier ;
    if result <= 0.0 { 0 }
    else if result >= u64::MAX as f64 { u64::MAX }
    else { result as u64 }

}
