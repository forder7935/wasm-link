
use crate::capnp::common::interface_capnp::interface_id ;



#[derive( Copy, Clone, Debug, Eq, Hash, PartialEq )]
pub struct InterfaceId( u64 );

impl InterfaceId {
    pub const fn new( id: u64 ) -> Self { Self( id )}
}

impl<'a> From<interface_id::Reader<'a>> for InterfaceId {
    fn from( reader: interface_id::Reader<'a> ) -> Self {
        Self( reader.get_id() )
    }
}

impl std::fmt::Display for InterfaceId {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(),std::fmt::Error> {
        std::fmt::Display::fmt( &self.0, f )
    }
}
