//! Interface metadata types and traits.
//!
//! An interface is a contract between plugins: it declares what functions an
//! implementer must export and what a consumer may import. Interfaces are not
//! tied to any specific plugin - they exist as abstract specifications that
//! plugins reference via their plugs and sockets.

use wit_parser::{ Function, FunctionKind };



/// Unique identifier for an interface.
///
/// Used to reference interfaces when building the plugin tree and linking
/// dependencies. Two plugins with the same `InterfaceId` in their plug/socket
/// declarations will be connected during loading.
#[derive( Copy, Clone, Debug, Eq, Hash, PartialEq )]
pub struct InterfaceId( u64 );

impl InterfaceId {
    /// Creates a new interface identifier from a `u64`.
    pub const fn new( id: u64 ) -> Self { Self( id )}
}

impl std::fmt::Display for InterfaceId {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(),std::fmt::Error> {
        std::fmt::Display::fmt( &self.0, f )
    }
}

impl From<InterfaceId> for u64 {
    fn from( id: InterfaceId ) -> Self { id.0 }
}

/// Trait for accessing interface metadata from a user-defined source.
///
/// Implement this trait to define how interface specifications are loaded. The source
/// can be anything: WIT files on disk, a database, embedded resources, or generated
/// at runtime. The framework uses this trait to discover interface contracts when
/// linking plugins together.
///
/// # Associated Types
///
/// - `Error`: The error type returned when metadata access fails
/// - `FunctionIter`: Iterator over the functions this interface declares
/// - `ResourceIter`: Iterator over the resource types this interface declares
pub trait InterfaceData: Sized {

    /// Error type for metadata access failures.
    type Error: std::error::Error ;
    /// Iterator over functions declared by this interface.
    type FunctionIter<'a>: IntoIterator<Item = &'a FunctionData> where Self: 'a ;
    /// Iterator over resource type names declared by this interface.
    type ResourceIter<'a>: IntoIterator<Item = &'a String> where Self: 'a ;

    /// Returns the unique identifier for this interface.
    fn id( &self ) -> InterfaceId ;

    /// Returns how many plugins may/must implement this interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn get_cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> ;

    /// Returns the WIT package name for this interface.
    ///
    /// # Errors
    /// Implementations may fail if the underlying data source is unavailable.
    fn get_package_name( &self ) -> Result<&str, Self::Error> ;

    /// Returns the functions exported by this interface.
    ///
    /// # Errors
    /// Implementations may fail if WIT parsing fails or the data source is unavailable.
    fn get_functions( &self ) -> Result<Self::FunctionIter<'_>, Self::Error> ;

    /// Returns the resource types defined by this interface.
    ///
    /// # Errors
    /// Implementations may fail if WIT parsing fails or the data source is unavailable.
    fn get_resources( &self ) -> Result<Self::ResourceIter<'_>, Self::Error> ;

}

/// Metadata about a function declared by an interface.
///
/// Contains information needed during linking to wire up cross-plugin dispatch,
/// including the function signature and return kind.
#[derive( Debug, Clone )]
pub struct FunctionData {
    function: Function,
    return_kind: ReturnKind,
}
impl FunctionData {

    /// Creates function metadata from a parsed WIT function and its return kind.
    pub fn new( function: Function, return_kind: ReturnKind ) -> Self {
        Self { function, return_kind }
    }

    /// Returns the function's name as declared in the interface.
    #[inline] pub fn name( &self ) -> &str { &self.function.name }
    /// Returns `true` if this function returns a value.
    #[inline] pub fn has_return( &self ) -> bool { self.return_kind != ReturnKind::Void }
    /// Returns the function's return kind.
    #[inline] pub fn return_kind( &self ) -> &ReturnKind { &self.return_kind }
    /// Returns `true` if this is a method (has a `self` parameter).
    #[inline] pub fn is_method( &self ) -> bool { match self.function.kind {
        FunctionKind::Freestanding | FunctionKind::Static( _ ) | FunctionKind::Constructor( _ ) => false,
        FunctionKind::Method( _ ) => true,
        FunctionKind::AsyncFreestanding | FunctionKind::AsyncMethod( _ ) | FunctionKind::AsyncStatic( _ )
        => unimplemented!( "Async functions are not yet implemented" ),
    }}

}

/// Categorizes a function's return for dispatch handling.
///
/// Determines how return values are processed during cross-plugin dispatch.
/// Resources require special wrapping to track ownership across plugin
/// boundaries, while plain data can be passed through directly.
///
/// # Choosing the Right Variant
///
/// **When uncertain, use [`MayContainResources`](Self::MayContainResources).** Using
/// `AssumeNoResources` when resources are actually present will cause resource handles
/// to be passed through unwrapped. This can lead to undefined behavior in plugins:
/// invalid handles, use-after-free, or calls dispatched to the wrong plugin.
///
/// `AssumeNoResources` is a performance optimization that skips the wrapping step.
/// Only use it when you are certain the return type contains no resource handles
/// anywhere in its structure (including nested within records, variants, lists, etc.).
#[derive( Debug, Clone, PartialEq )]
pub enum ReturnKind {
    /// Function returns nothing (void).
    Void,
    /// Assumes no resource handles are present - skips wrapping for performance.
    ///
    /// **Warning:** Only use this if you are certain no resources are present.
    /// If resources are returned but this variant is used, resource handles will
    /// not be wrapped correctly, potentially causing undefined behavior in plugins.
    /// When in doubt, use [`MayContainResources`](Self::MayContainResources) instead.
    AssumeNoResources,
    /// Function may return resource handles - always wraps safely.
    ///
    /// Use this variant whenever resources might be present in the return value,
    /// or when you're unsure. The performance overhead of wrapping is preferable
    /// to the undefined behavior caused by unwrapped resource handles.
    MayContainResources,
}

/// Specifies how many plugins may or must implement an interface.
///
/// Cardinality expresses what the consumer of an interface expects:
///
/// - `ExactlyOne`: The consumer expects a single implementation. Dispatch returns
///   a single value directly.
///
/// - `AtMostOne`: The consumer can work with zero or one implementation. Dispatch
///   returns an `Option`.
///
/// - `AtLeastOne`: The consumer requires at least one implementation but can handle
///   multiple. Dispatch returns a collection.
///
/// - `Any`: The consumer doesn't care how many implementations exist (including zero).
///   Dispatch returns a collection. Useful for optional extension points.
///
/// The cardinality determines the [`Socket`] variant used at runtime and affects
/// how dispatch results are wrapped.
///
/// [`Socket`]: crate::Socket
#[derive( Debug, PartialEq, Eq, Copy, Clone )]
pub enum InterfaceCardinality {
    /// Zero or one plugin allowed. Dispatch returns `Option<T>`.
    AtMostOne,
    /// Exactly one plugin required. Dispatch returns `T` directly.
    ExactlyOne,
    /// One or more plugins required. Dispatch returns a collection.
    AtLeastOne,
    /// Zero or more plugins allowed. Dispatch returns a collection.
    Any,
}
impl std::fmt::Display for InterfaceCardinality {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> std::fmt::Result { write!( f, "{:?}", self )}
}
