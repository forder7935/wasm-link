use wasmtime::component::{ Component, Instance, Val };
use wasmtime::Store ;

use crate::plugin::{ PluginId, PluginData };
use crate::loading::DispatchError ;



pub struct PluginInstance<T: PluginData + 'static> {
    pub(crate) id: PluginId,
    pub(crate) _component: Component,
    pub(crate) store: Store<T>,
    pub(crate) instance: Instance,
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

    const PLACEHOLDER_VAL: Val = Val::Tuple( vec![] );

    pub(crate) fn dispatch<E: std::error::Error>(
        &mut self,
        interface_path: &str,
        function: &str,
        returns: bool,
        data: &[Val],
    ) -> Result<Val, DispatchError<E>> {

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
