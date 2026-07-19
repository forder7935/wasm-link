use thiserror::Error;

use crate::{ResourceCreationError, ResourceReceiveError};

/// Errors produced by synchronous dispatch.
#[derive(Error, Debug)]
pub enum DispatchError {
    /// The interface path is unknown.
    #[error("Invalid Interface Path: {0}")]
    InvalidInterfacePath(String),
    /// The function is unknown.
    #[error("Invalid Function: {0}")]
    InvalidFunction(String),
    /// A required return value was absent.
    #[error("Missing Response")]
    MissingResponse,
    /// WebAssembly execution failed.
    #[error("Runtime Exception")]
    RuntimeException(wasmtime::Error),
    /// Arguments did not match the function signature.
    #[error("Invalid Argument List")]
    InvalidArgumentList,
    /// The value uses an unsupported Component Model type.
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
    /// A resource handle could not be created.
    #[error("Resource Create Error: {0}")]
    ResourceCreationError(#[from] ResourceCreationError),
    /// A resource handle could not be received.
    #[error("Resource Receive Error: {0}")]
    ResourceReceiveError(#[from] ResourceReceiveError),
}

impl From<crate::plugin_instance::DispatchError> for DispatchError {
    fn from(error: crate::plugin_instance::DispatchError) -> Self {
        match error {
            crate::plugin_instance::DispatchError::InvalidInterfacePath(value) => {
                Self::InvalidInterfacePath(value)
            }
            crate::plugin_instance::DispatchError::InvalidFunction(value) => {
                Self::InvalidFunction(value)
            }
            crate::plugin_instance::DispatchError::MissingResponse => Self::MissingResponse,
            crate::plugin_instance::DispatchError::RuntimeException(value) => {
                Self::RuntimeException(value)
            }
            crate::plugin_instance::DispatchError::InvalidArgumentList => Self::InvalidArgumentList,
            crate::plugin_instance::DispatchError::UnsupportedType(value) => {
                Self::UnsupportedType(value)
            }
            crate::plugin_instance::DispatchError::ResourceCreationError(value) => {
                Self::ResourceCreationError(value)
            }
            crate::plugin_instance::DispatchError::ResourceReceiveError(value) => {
                Self::ResourceReceiveError(value)
            }
            crate::plugin_instance::DispatchError::ExecutorUnavailable
            | crate::plugin_instance::DispatchError::DispatchQueueFull => {
                // Synchronous dispatchers cannot construct either variant. Reaching
                // this arm means an internal runtime boundary was violated.
                panic!("async-only error escaped the synchronous runtime")
            }
        }
    }
}

impl From<DispatchError> for wasmtime::component::Val {
    fn from(error: DispatchError) -> Self {
        match error {
            DispatchError::InvalidInterfacePath(value) => Self::Variant(
                "invalid-interface-path".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::InvalidFunction(value) => Self::Variant(
                "invalid-function".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::MissingResponse => Self::Variant("missing-response".to_string(), None),
            DispatchError::RuntimeException(value) => Self::Variant(
                "runtime-exception".to_string(),
                Some(Box::new(Self::String(value.to_string()))),
            ),
            DispatchError::InvalidArgumentList => {
                Self::Variant("invalid-argument-list".to_string(), None)
            }
            DispatchError::UnsupportedType(value) => Self::Variant(
                "unsupported-type".to_string(),
                Some(Box::new(Self::String(value))),
            ),
            DispatchError::ResourceCreationError(value) => value.into(),
            DispatchError::ResourceReceiveError(value) => value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::plugin_instance::DispatchError as CoreDispatchError;
    use crate::{ResourceCreationError, ResourceReceiveError};
    use wasmtime::component::Val;

    #[test]
    fn converts_every_synchronous_internal_error() {
        [
            CoreDispatchError::InvalidInterfacePath("path".to_string()),
            CoreDispatchError::InvalidFunction("function".to_string()),
            CoreDispatchError::MissingResponse,
            CoreDispatchError::RuntimeException(wasmtime::Error::msg("trap")),
            CoreDispatchError::InvalidArgumentList,
            CoreDispatchError::UnsupportedType("future".to_string()),
            CoreDispatchError::ResourceCreationError(ResourceCreationError::ResourceTableFull),
            CoreDispatchError::ResourceReceiveError(ResourceReceiveError::InvalidHandle),
        ]
        .into_iter()
        .map(super::DispatchError::from)
        .map(Val::from)
        .for_each(drop);
    }

    #[test]
    #[should_panic(expected = "async-only error escaped the synchronous runtime")]
    fn rejects_async_only_internal_errors() {
        let _ = super::DispatchError::from(
            crate::plugin_instance::DispatchError::DispatchQueueFull,
        );
    }
}
