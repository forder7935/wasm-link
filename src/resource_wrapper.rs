use std::sync::Arc ;
use thiserror::Error ;
use wasmtime::component::{ Resource, ResourceAny, Val };
use wasmtime::StoreContextMut ;

use crate::PluginContext ;



#[derive( Debug )]
pub(crate) struct ResourceWrapper<Id> {
    pub plugin_id: Id,
    pub resource_handle: ResourceAny,
}

/// Errors that occur when creating a resource handle for cross-plugin transfer.
///
/// Resources are wrapped before being passed between plugins to track ownership.
/// These errors indicate failures in that wrapping process.
#[derive( Debug, Error )]
pub enum ResourceCreationError {
    /// The resource table has reached capacity and cannot store more handles.
    #[error( "Resource Table Full" )] ResourceTableFull,
    /// Failed to convert a stored resource into a host handle.
    #[error( "Resource Handle Conversion Failed" )] ResourceHandleConversionFailed,
}
impl From<ResourceCreationError> for Val {
    fn from( error: ResourceCreationError ) -> Self { match error {
        ResourceCreationError::ResourceTableFull => Val::Variant( "resource-table-full".to_string(), None ),
        ResourceCreationError::ResourceHandleConversionFailed => Val::Variant( "resource-handle-conversion-failed".to_string(), None ),
    }}
}

/// Errors that occur when unwrapping a resource handle received from another plugin.
///
/// When a plugin receives a resource from another plugin, the handle must be
/// looked up in the resource table to retrieve the original resource.
/// These errors indicate failures in that lookup process.
#[derive( Debug, Error )]
pub enum ResourceReceiveError {
    /// The handle doesn't correspond to any known resource (possibly already dropped or invalid).
    #[error( "Invalid Handle" )] InvalidHandle,
}
impl From<ResourceReceiveError> for Val {
    fn from( error: ResourceReceiveError ) -> Self { match error {
        ResourceReceiveError::InvalidHandle => Val::Variant( "invalid-resource-handle".to_string(), None ),
    }}
}

impl<Id: 'static + Send + Sync> ResourceWrapper<Id> {

    /// Wraps a resource handle with the owning plugin's id.
    pub(crate) fn new( plugin_id: Id, resource_handle: ResourceAny ) -> Self {
        Self { plugin_id, resource_handle }
    }

    /// Stores the wrapped resource in the host table and returns a handle.
    pub(crate) fn attach<Ctx: PluginContext>(
        self,
        store: &mut StoreContextMut<Ctx>,
    ) -> Result<ResourceAny, ResourceCreationError> {
        let table = store.data_mut().resource_table();
        let resource = table.push( Arc::new( self )).map_err(|_| ResourceCreationError::ResourceTableFull )?;
        ResourceAny::try_from_resource( resource, store )
            .map_err(|_| ResourceCreationError::ResourceHandleConversionFailed )
    }

    /// Looks up a wrapped resource by handle in the host resource table.
    pub(crate) fn from_handle<'a, Ctx: PluginContext>(
        handle: ResourceAny,
        store: &'a mut StoreContextMut<Ctx>,
    ) -> Result<&'a Self, ResourceReceiveError> {
        let resource = Resource::<Arc<Self>>::try_from_resource_any( handle, &mut *store ).map_err(|_| ResourceReceiveError::InvalidHandle )?;
        let table = store.data_mut().resource_table();
        let wrapped = table.get( &resource ).map_err(|_| ResourceReceiveError::InvalidHandle )?;
        Ok( wrapped )
    }

    /// Drops a wrapped resource by handle from the host resource table.
    pub(crate) fn drop<Ctx: PluginContext>( mut ctx: StoreContextMut<Ctx>, handle: u32 ) -> Result<(), wasmtime::Error> {
        let resource = Resource::<Arc<Self>>::new_own( handle );
        let table = ctx.data_mut().resource_table();
        table.delete( resource ).map_err(|_| wasmtime::Error::new( ResourceReceiveError::InvalidHandle ))?;
        Ok(())
    }

}
