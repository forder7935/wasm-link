use wasmtime::Engine ;
use wasmtime::component::Component ;

use crate::InterfaceId ;



/// Unique identifier for a plugin.
#[derive( Eq, Hash, PartialEq, Debug, Clone, Copy )]
pub struct PluginId( u64 );

impl PluginId {
    pub const fn new( id: u64 ) -> Self { Self( id ) }
}

impl std::fmt::Display for PluginId {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(), std::fmt::Error> {
        std::fmt::Display::fmt( &self.0, f )
    }
}

impl From<PluginId> for u64 {
    fn from( id: PluginId ) -> Self { id.0 }
}

/// Trait for accessing plugin metadata from a user-defined source (filesystem, database, etc.).
pub trait PluginData: Sized {

    type Error: std::error::Error ;
    type SocketIter<'a>: IntoIterator<Item = &'a InterfaceId> where Self: 'a ;

    /// Returns this plugin's unique identifier.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable
    fn get_id( &self ) -> Result<&PluginId, Self::Error> ;

    /// Returns the interface ID that this plugin implements (its "plug").
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable
    fn get_plug( &self ) -> Result<&InterfaceId, Self::Error> ;

    /// Returns the interface IDs that this plugin depends on (its "sockets").
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable
    fn get_sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> ;

    /// Compiles this plugin's WASM binary into a wasmtime Component.
    ///
    /// # Errors
    /// Implementations may fail due to any errors when reading the WASM source,
    /// or wasmtime compilation errors if the binary is invalid.
    fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> ;

}
