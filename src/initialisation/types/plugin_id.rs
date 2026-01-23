
use crate::capnp::common::plugin_capnp::plugin_id ;



#[derive( Eq, Hash, PartialEq, Debug, Clone )]
pub struct PluginId( String );

impl PluginId {
    pub const fn new( id: String ) -> Self { Self( id ) }
}

impl<'a> TryFrom<plugin_id::Reader<'a>> for PluginId {

    type Error = capnp::Error ;

    fn try_from( reader: plugin_id::Reader<'a> ) -> Result<Self, Self::Error> {
        Ok( Self( reader.get_id()?.to_string()? ))
    }

}

impl std::fmt::Display for PluginId {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(), std::fmt::Error> {
        std::fmt::Display::fmt( &self.0, f )
    }
}

impl AsRef<std::path::Path> for PluginId {
    fn as_ref( &self ) -> &std::path::Path {
        AsRef::<std::path::Path>::as_ref( &self.0 )
    }
}
