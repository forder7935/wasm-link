

#[derive( Eq, Hash, PartialEq, Debug, Clone )]
pub struct PluginId( String );

impl PluginId {
    pub const fn new( id: String ) -> Self { Self( id ) }
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
