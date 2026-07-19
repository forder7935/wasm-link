use std::collections::{HashMap, HashSet};

use crate::interface::{Function as FunctionMetadata, Interface as InterfaceMetadata};
use crate::{FunctionKind, ReturnKind};

/// Metadata for a synchronous WIT function.
///
/// This type deliberately has no asynchronous constructor or state.
#[derive(Debug, Clone)]
pub struct Function {
    metadata: FunctionMetadata,
}

impl Function {
    /// Creates metadata for a synchronous WIT function.
    pub fn new(kind: FunctionKind, return_kind: ReturnKind) -> Self {
        Self {
            metadata: FunctionMetadata::new(kind, return_kind),
        }
    }

    /// The function's return kind.
    pub fn return_kind(&self) -> ReturnKind {
        self.metadata.return_kind()
    }

    /// Whether this is a freestanding function or resource method.
    pub fn kind(&self) -> FunctionKind {
        self.metadata.kind()
    }

    pub(crate) fn metadata(&self) -> &FunctionMetadata {
        &self.metadata
    }
    pub(crate) fn from_metadata(metadata: &FunctionMetadata) -> Self {
        Self {
            metadata: metadata.clone(),
        }
    }
}

/// A synchronous WIT interface declaration.
#[derive(Debug, Clone, Default)]
pub struct Interface {
    metadata: InterfaceMetadata,
}

impl Interface {
    /// Creates an interface containing only synchronous functions.
    pub fn new(functions: HashMap<String, Function>, resources: HashSet<String>) -> Self {
        Self {
            metadata: InterfaceMetadata::new(
                functions
                    .into_iter()
                    .map(|(name, function)| (name, function.metadata))
                    .collect(),
                resources,
            ),
        }
    }

    pub(crate) fn into_metadata(self) -> InterfaceMetadata {
        self.metadata
    }
}
