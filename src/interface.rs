use wit_parser::{ Function, FunctionKind };



/// Unique identifier for an interface.
#[derive( Copy, Clone, Debug, Eq, Hash, PartialEq )]
pub struct InterfaceId( u64 );

impl InterfaceId {
    pub const fn new( id: u64 ) -> Self { Self( id )}
}

impl std::fmt::Display for InterfaceId {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(),std::fmt::Error> {
        std::fmt::Display::fmt( &self.0, f )
    }
}

/// Trait for accessing interface metadata from a user-defined source (filesystem, database, etc.).
pub trait InterfaceData: Sized {

    type Error: std::error::Error ;
    type FunctionIter<'a>: IntoIterator<Item = &'a FunctionData> where Self: 'a ;
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

/// Metadata about a function exported by an interface.
///
/// Typically constructed from WIT definitions. Used during linking to wire up
/// cross-plugin dispatch.
#[derive( Debug, Clone )]
pub struct FunctionData {
    function: Function,
    return_type: FunctionReturnType,
}
impl FunctionData {

    pub fn new( function: Function, return_type: FunctionReturnType ) -> Self {
        Self { function, return_type }
    }

    #[inline] pub fn name( &self ) -> &str { &self.function.name }
    #[inline] pub fn has_return( &self ) -> bool { self.return_type != FunctionReturnType::None }
    #[inline] pub fn return_type( &self ) -> &FunctionReturnType { &self.return_type }
    #[inline] pub fn is_method( &self ) -> bool { match self.function.kind {
        FunctionKind::Freestanding | FunctionKind::Static( _ ) | FunctionKind::Constructor( _ ) => false,
        FunctionKind::Method( _ ) => true,
        FunctionKind::AsyncFreestanding | FunctionKind::AsyncMethod( _ ) | FunctionKind::AsyncStatic( _ )
        => unimplemented!( "Async functions are not yet implemented" ),
    }}

}

/// Categorizes a function's return type for dispatch handling.
///
/// Resources require special wrapping to track ownership across plugin boundaries.
#[derive( Debug, Clone, PartialEq )]
pub enum FunctionReturnType {
    None,
    DataNoResource,
    DataWithResources,
}

/// Specifies how many plugins may or must implement an interface.
///
/// - `AtMostOne` - Zero or one plugin allowed
/// - `ExactlyOne` - Exactly one plugin required
/// - `AtLeastOne` - One or more plugins required
/// - `Any` - Zero or more plugins allowed
#[derive( Debug, PartialEq, Eq, Copy, Clone )]
pub enum InterfaceCardinality {
    AtMostOne,
    ExactlyOne,
    AtLeastOne,
    Any,
}
impl std::fmt::Display for InterfaceCardinality {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> std::fmt::Result { write!( f, "{:?}", self )}
}
