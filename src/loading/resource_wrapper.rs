use std::sync::{ Arc, LazyLock, Mutex };
use thiserror::Error ;
use wasmtime::component::{ Resource, ResourceAny, ResourceTable, Val };
use wasmtime::{ AsContextMut, StoreContextMut };

use crate::plugin::PluginData ;



static RESOURCE_TABLE: LazyLock<Mutex<ResourceTable>> = LazyLock::new(|| Mutex::new( ResourceTable::new() ));

pub(super) struct ResourceWrapper<Id> {
    pub plugin_id: Id,
    pub resource_handle: ResourceAny,
}

#[derive( Debug, Error )]
pub enum ResourceCreationError {
    #[error( "Lock Rejected" )] LockRejected,
    #[error( "Resource Table Full" )] ResourceTableFull,
}
impl From<ResourceCreationError> for Val {
    fn from( error: ResourceCreationError ) -> Self { match error {
        ResourceCreationError::LockRejected => Val::Variant( "resource-table-lock-rejected".to_string(), None ),
        ResourceCreationError::ResourceTableFull => Val::Variant( "resource-table-full".to_string(), None ),
    }}
}

#[derive( Debug, Error )]
pub enum ResourceReceiveError {
    #[error( "Lock Rejected" )] LockRejected,
    #[error( "Invalid Handle" )] InvalidHandle,
}
impl From<ResourceReceiveError> for Val {
    fn from( error: ResourceReceiveError ) -> Self { match error {
        ResourceReceiveError::LockRejected => Val::Variant( "resource-table-lock-rejected".to_string(), None ),
        ResourceReceiveError::InvalidHandle => Val::Variant( "invalid-resource-handle".to_string(), None ),
    }}
}

impl<Id: 'static + std::fmt::Debug> ResourceWrapper<Id> {

    pub fn new( plugin_id: Id, resource_handle: ResourceAny ) -> Self {
        Self { plugin_id, resource_handle }
    }

    pub fn attach(
        self,
        store: &mut impl AsContextMut,
    ) -> Result<ResourceAny, ResourceCreationError> where Id: Send + Sync {
        let mut lock = RESOURCE_TABLE.lock().map_err(|_| ResourceCreationError::LockRejected )?;
        let resource = lock.push( Arc::new( self )).map_err(|_| ResourceCreationError::ResourceTableFull )?;
        let temp = ResourceAny::try_from_resource( resource, store ).map_err(|_| unreachable!( "Resource already taken" ));
        temp
    }

    pub fn from_handle(
        handle: ResourceAny,
        store: &mut impl AsContextMut,
    ) -> Result<Arc<Self>, ResourceReceiveError> {
        let lock = RESOURCE_TABLE.lock().map_err(|_| ResourceReceiveError::LockRejected )?;
        let resource = Resource::try_from_resource_any( handle, store ).map_err(|_| ResourceReceiveError::InvalidHandle )?;
        let wrapped = lock.get( &resource ).map_err(|_| ResourceReceiveError::InvalidHandle )?;
        Ok( Arc::clone( wrapped ))
    }

    pub fn drop<T: PluginData>( _: StoreContextMut<T>, handle: u32 ) -> Result<(), wasmtime::Error> {
        let resource = Resource::<Arc<Self>>::new_own( handle );
        let mut lock = RESOURCE_TABLE.lock().map_err(|_| wasmtime::Error::new( ResourceReceiveError::LockRejected ))?;
        lock.delete( resource ).map_err(|_| wasmtime::Error::new( ResourceReceiveError::InvalidHandle ))?;
        Ok(())
    }

}
