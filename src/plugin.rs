//! Plugin metadata types and traits.
//!
//! A plugin is a WASM component that implements one interface (its **plug**) and
//! may depend on zero or more other interfaces (its **sockets**). The plug declares
//! what the plugin exports; sockets declare what the plugin expects to import from
//! other plugins.

use wasmtime::Engine ;
use wasmtime::component::{ Component, ResourceTable } ;

/// Trait for accessing a [`ResourceTable`] from the store's data type.
///
/// Resources that flow between plugins need to be wrapped to track ownership.
/// This trait provides access to the table where those wrapped resources are stored.
///
/// # Example
///
/// ```
/// use wasmtime::component::ResourceTable ;
/// use wasm_link::PluginCtxView ;
///
/// struct MyPluginData {
///     resource_table: ResourceTable,
///     // ... other fields
/// }
///
/// impl PluginCtxView for MyPluginData {
///     fn resource_table( &mut self ) -> &mut ResourceTable {
///         &mut self.resource_table
///     }
/// }
/// ```
pub trait PluginCtxView {
    /// Returns a mutable reference to the resource table.
    fn resource_table( &mut self ) -> &mut ResourceTable ;
}

/// Trait for accessing plugin metadata and WASM binaries from a user-defined source.
///
/// Implement this trait to define how plugins are discovered, how their metadata
/// is read, and how their WASM binaries are loaded.
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
/// - `Id`: Unique identifier type for plugins (e.g., `String`, `Uuid`, `PathBuf`)
/// - `InterfaceId`: Must match the `Id` type used by your [`InterfaceData`]( crate::InterfaceData ) implementation
/// - `Error`: The error type returned when metadata access or compilation fails
/// - `SocketIter`: Iterator over the interface IDs this plugin depends on
pub trait PluginData: Sized + Send + PluginCtxView {

    /// A type used as a unique identifier for a plugin
    type Id: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync ;
    /// A type used as a unique identifier for an interface
    type InterfaceId: Clone + std::hash::Hash + Eq + std::fmt::Display ;
    /// Error type for metadata access and compilation failures.
    type Error: std::error::Error ;
    /// Iterator over interface IDs this plugin depends on (its sockets).
    type SocketIter<'a>: IntoIterator<Item = &'a Self::InterfaceId> where Self: 'a ;

    /// Returns this plugin's unique identifier.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn id( &self ) -> Result<&Self::Id, Self::Error> ;

    /// Returns the interface ID that this plugin implements (its plug).
    ///
    /// The plug declares which interface this plugin provides an implementation for.
    /// The plugin must export all functions declared by this interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn plug( &self ) -> Result<&Self::InterfaceId, Self::Error> ;

    /// Returns the interface IDs that this plugin depends on (its sockets).
    ///
    /// Each socket is an interface the plugin expects to call into. During linking,
    /// these calls are wired to the plugin(s) implementing each interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> ;

    /// Compiles this plugin's WASM binary into a wasmtime Component.
    ///
    /// Called during [`PluginTree::load`]( crate::PluginTree::load ) to compile the plugin. The implementation
    /// is responsible for locating and reading the WASM binary.
    ///
    /// # Errors
    /// May fail due to I/O errors reading the WASM source, or wasmtime compilation
    /// errors if the binary is invalid or incompatible.
    fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> ;

}
