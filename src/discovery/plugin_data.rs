use wasmtime::Engine ;
use wasmtime::component::Component ;

use crate::{ InterfaceId, PluginId };



pub trait PluginData: Sized {

    type Error: std::error::Error ;
    type SocketIter<'a>: IntoIterator<Item = &'a InterfaceId> where Self: 'a ;

    fn get_id( &self ) -> Result<&PluginId, Self::Error> ;
    fn get_plug( &self ) -> Result<&InterfaceId, Self::Error> ;
    fn get_sockets<'a>( &'a self ) -> Result<Self::SocketIter<'a>, Self::Error> ;

    fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> ;

}
