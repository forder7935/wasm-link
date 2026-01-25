use wit_parser::{ Function, FunctionKind };

use crate::InterfaceId ;



pub trait InterfaceData: Sized {

    type Error: std::error::Error ;
    type FunctionIter<'a>: IntoIterator<Item = &'a FunctionData> where Self: 'a ;
    type ResourceIter<'a>: IntoIterator<Item = &'a String> where Self: 'a ;

    fn new( id: InterfaceId ) -> Result<Self, Self::Error> ;

    fn get_cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> ;
    fn get_package_name( &self ) -> Result<&str, Self::Error> ;
    fn get_functions<'a>( &'a self ) -> Result<Self::FunctionIter<'a>, Self::Error> ;
    fn get_resources<'a>( &'a self ) -> Result<Self::ResourceIter<'a>, Self::Error> ;

}

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

#[derive( Debug, Clone, PartialEq )]
pub enum FunctionReturnType {
    None,
    DataNoResource,
    DataWithResources,
}

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
