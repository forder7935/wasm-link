use std::collections::HashMap ;
use std::sync::{ Arc, RwLock };
use wasmtime::Engine ;
use wasmtime::component::Linker ;

mod discovery ;
mod loading ;
mod types ;

pub use types::{ InterfaceId, PluginId };
pub use discovery::{ InterfaceData, PluginData, InterfaceCardinality, FunctionData, FunctionReturnType };
pub use loading::{ Socket, PluginContext, PreloadError };
use crate::utils::{ PartialSuccess, PartialResult };
use discovery::discover_all ;
use loading::{ preload_plugin_tree, PluginInstance };



pub struct PluginTreeHead<I: InterfaceData, P: PluginData + 'static> {
    _interface: Arc<I>,
    socket: Arc<Socket<RwLock<PluginInstance<P>>>>,
}

impl<I: InterfaceData, P: PluginData> PluginTreeHead<I, P> { }

pub struct PluginTree<I: InterfaceData, P: PluginData> {
    root_interface_id: InterfaceId,
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
}
impl<I: InterfaceData, P: PluginData> PluginTree<I, P> {
    
    pub fn new<E, IE, PE>(
        plugins: Vec<P>,
        root_interface_id: InterfaceId,
    ) -> PartialSuccess<Self, E>
    where 
        IE: std::error::Error,
        PE: std::error::Error,
        I: InterfaceData<Error = IE> + Sized,
        P: PluginData<Error = PE> + Sized,
        E: From<IE> + From<PE>,
    {
        let ( socket_map, errors ) = discover_all::<I, _, _, _, _>( plugins, &root_interface_id );
        ( Self { root_interface_id, socket_map }, errors )
    }

    pub fn load<IE: std::error::Error, PE: std::error::Error>(
        self,
        engine: Engine,
        exports: &Linker<PluginContext<P>>,
    ) -> PartialResult<PluginTreeHead<I, P>, PreloadError<IE, PE>, PreloadError<IE, PE>>
    where 
        IE: std::error::Error,
        PE: std::error::Error,
        I: InterfaceData<Error = IE> + Sized,
        P: PluginData<Error = PE> + Sized + Send + Sync,
    {
        match preload_plugin_tree( self.socket_map, &engine, exports, self.root_interface_id ) {
            Ok((( _interface, socket ), errors )) => Ok(( PluginTreeHead { _interface, socket }, errors )),
            Err(( err, errors )) => Err(( err , errors )),
        }
    }

}

