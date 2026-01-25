use std::collections::HashMap ;
use std::sync::{ Arc, RwLock };
use wasmtime::Engine ;
use wasmtime::component::Linker ;

use crate::types::InterfaceId ;
use crate::discovery::{ InterfaceData, PluginData, discover_all };
use crate::loading::{ Socket, PluginContext, PreloadError, preload_plugin_tree, PluginInstance };
use crate::utils::{ PartialSuccess, PartialResult };



pub struct PluginTreeHead<I: InterfaceData, P: PluginData + 'static> {
    _interface: Arc<I>,
    pub(crate) socket: Arc<Socket<RwLock<PluginInstance<P>>>>,
}

pub struct PluginTree<I: InterfaceData, P: PluginData> {
    root_interface_id: InterfaceId,
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
}

impl<I: InterfaceData, P: PluginData> PluginTree<I, P> {

    pub fn new<E>(
        plugins: Vec<P>,
        root_interface_id: InterfaceId,
    ) -> PartialSuccess<Self, E>
    where
        E: From<I::Error> + From<P::Error>,
    {
        let ( socket_map, errors ) = discover_all::<I, P, E>( plugins, &root_interface_id );
        ( Self { root_interface_id, socket_map }, errors )
    }

    pub fn load(
        self,
        engine: &Engine,
        exports: &Linker<PluginContext<P>>,
    ) -> PartialResult<PluginTreeHead<I, P>, PreloadError<I::Error, P::Error>, PreloadError<I::Error, P::Error>>
    where
        P: Send + Sync,
    {
        match preload_plugin_tree( self.socket_map, engine, exports, self.root_interface_id ) {
            Ok((( _interface, socket ), errors )) => Ok(( PluginTreeHead { _interface, socket }, errors )),
            Err(( err, errors )) => Err(( err , errors )),
        }
    }

}
