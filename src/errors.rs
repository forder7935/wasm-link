//! Errors at binding selection, plugin invocation, and dependency routing boundaries.
use crate::{ResourceCreationError, ResourceReceiveError};
use thiserror::Error;
use wasmtime::component::Val;

/// A requested operation is absent from the binding's metadata.
/// These are the outer errors from both forms of `Binding::dispatch`.
#[derive(Debug, Error)]
pub enum DispatchError {
	/// The binding does not declare this interface.
	#[error("Invalid Interface Path: {0}")]
	InvalidInterfacePath(String),
	/// The binding does not declare this function.
	#[error("Invalid Function: {0}")]
	InvalidFunction(String),
}

/// Failure invoking one plugin. Used by asynchronous dispatch and by
/// [`SyncCallError::Call`] in synchronous dispatch.
#[derive(Debug, Error)]
pub enum CallError {
	/// The actual component does not export the requested interface.
	#[error("Invalid Interface Path: {0}")]
	InvalidInterfacePath(String),
	/// The actual component does not export the requested function.
	#[error("Invalid Function: {0}")]
	InvalidFunction(String),
	/// Wasmtime rejected the invocation, execution trapped, or the store failed.
	#[error("Runtime Exception: {0}")]
	RuntimeException(#[source] wasmtime::Error),
	/// A value contains a future, stream, or error-context, which wasm-link cannot transfer.
	#[error("Unsupported type: {0}")]
	UnsupportedType(String),
}

/// Failure invoking one plugin through synchronous dispatch.
/// No cancellation or asynchronous scheduling variants are exposed here.
#[derive(Debug, Error)]
pub enum SyncCallError {
	/// Another invocation owns or is queued for the plugin. The call did not start.
	/// Retrying from a callback into the same active plugin cannot succeed until
	/// the enclosing invocation returns.
	#[error("Plugin busy")]
	PluginBusy,
	/// The plugin could not execute the requested operation.
	#[error(transparent)]
	Call(#[from] CallError),
}

#[derive(Debug, Error)]
pub(crate) enum RouteError {
	#[error(transparent)]
	Call(#[from] CallError),
	#[error("Plugin busy")]
	PluginBusy,
	#[error("Invalid argument list")]
	InvalidArgumentList,
	#[error(transparent)]
	ResourceCreation(#[from] ResourceCreationError),
	#[error(transparent)]
	ResourceReceive(#[from] ResourceReceiveError),
}
impl From<CallError> for Val {
	fn from(error: CallError) -> Self {
		let (name, detail) = match error {
			CallError::InvalidInterfacePath(s) => ("invalid-interface-path", s),
			CallError::InvalidFunction(s) => ("invalid-function", s),
			CallError::RuntimeException(e) => ("runtime-exception", format!("{e:#}")),
			CallError::UnsupportedType(s) => ("unsupported-type", s),
		};
		Val::Variant(name.into(), Some(Box::new(Val::String(detail))))
	}
}
impl From<RouteError> for Val {
	fn from(error: RouteError) -> Self {
		match error {
			RouteError::Call(e) => e.into(),
			RouteError::PluginBusy => Val::Variant("plugin-busy".into(), None),
			RouteError::InvalidArgumentList => Val::Variant("invalid-argument-list".into(), None),
			RouteError::ResourceCreation(e) => e.into(),
			RouteError::ResourceReceive(e) => e.into(),
		}
	}
}
impl From<SyncCallError> for Val {
	fn from(error: SyncCallError) -> Self {
		match error {
			SyncCallError::PluginBusy => RouteError::PluginBusy.into(),
			SyncCallError::Call(e) => e.into(),
		}
	}
}

/// The requested plugin is not in this binding. No resources were released.
/// Recover the value and release it through its owning plugin's binding.
#[derive(Debug, Error)]
#[error("plugin is not in this binding; resources were not released")]
pub struct ResourceReleaseError {
	/// The original value, whose resource ownership remains with the caller.
	pub value: Val,
}
