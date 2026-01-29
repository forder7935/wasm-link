//! Plugin metadata types and traits.
//!
//! A plugin is a WASM component that implements one interface (its **plug**) and
//! may depend on zero or more other interfaces (its **sockets**). The plug declares
//! what the plugin exports; sockets declare what the plugin expects to import from
//! other plugins.

use wasmtime::Engine ;
use wasmtime::component::Component ;

use crate::InterfaceId ;



/// Unique identifier for a plugin.
///
/// Used to track plugins throughout the loading process and to identify specific
/// plugins in multi-plugin sockets (when cardinality allows multiple implementations).
#[derive( Eq, Hash, PartialEq, Debug, Clone, Copy )]
pub struct PluginId( u64 );

impl PluginId {
    /// Creates a new plugin identifier from a `u64`.
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

/// Trait for accessing plugin metadata from a user-defined source.
///
/// Implement this trait to define how plugin specifications and WASM binaries are
/// loaded. The source can be anything: files on disk, a database, network resources,
/// or embedded binaries.
///
/// Each plugin declares:
/// - A **plug**: the interface it implements (what it exports)
/// - Zero or more **sockets**: interfaces it depends on (what it imports)
///
/// During loading, the framework uses this information to build the dependency graph
/// and link plugins together.
///
/// # Associated Types
///
/// - `Error`: The error type returned when metadata access or compilation fails
/// - `SocketIter`: Iterator over the interface IDs this plugin depends on
pub trait PluginData: Sized {

    /// Error type for metadata access and compilation failures.
    type Error: std::error::Error ;
    /// Iterator over interface IDs this plugin depends on (its sockets).
    type SocketIter<'a>: IntoIterator<Item = &'a InterfaceId> where Self: 'a ;

    /// Returns this plugin's unique identifier.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn get_id( &self ) -> Result<&PluginId, Self::Error> ;

    /// Returns the interface ID that this plugin implements (its plug).
    ///
    /// The plug declares which interface this plugin provides an implementation for.
    /// The plugin must export all functions declared by this interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn get_plug( &self ) -> Result<&InterfaceId, Self::Error> ;

    /// Returns the interface IDs that this plugin depends on (its sockets).
    ///
    /// Each socket is an interface the plugin expects to call into. During linking,
    /// these calls are wired to the plugin(s) implementing each interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn get_sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> ;

    /// Compiles this plugin's WASM binary into a wasmtime Component.
    ///
    /// Called during [`PluginTree::load`] to compile the plugin. The implementation
    /// is responsible for locating and reading the WASM binary.
    ///
    /// # Errors
    /// May fail due to I/O errors reading the WASM source, or wasmtime compilation
    /// errors if the binary is invalid or incompatible.
    ///
    /// [`PluginTree::load`]: crate::PluginTree::load
    fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> ;

}
