use wasmtime::component::{ Component, Instance };
use wasmtime::Store ;

use crate::PluginId ;
use super::PluginData ;



pub struct PluginInstance<T: PluginData + 'static> {
    pub(super) id: PluginId,
    pub(super) _component: Component,
    pub(super) store: Store<T>,
    pub(super) instance: Instance,
}

impl<T: PluginData + std::fmt::Debug> std::fmt::Debug for PluginInstance<T> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct( "Plugin Instance" )
            .field( "id", &self.id )
            .field( "data", &self.store.data() )
            .field( "store", &self.store )
            .finish_non_exhaustive()
    }
}

impl<T: PluginData> PluginInstance<T> {
    pub fn id( &self ) -> &PluginId { &self.id }
}
