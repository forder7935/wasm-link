
/// Chainable append operations for collections.
///
/// Provides a more functional style than `push()`/`extend()`, hiding mutation
/// behind a helper that returns `Self`.
pub trait Merge<T> {
    fn merge( self, value: T ) -> Self ;
    fn merge_all( self, values: impl IntoIterator<Item = T> ) -> Self ;
}

impl<T> Merge<T> for Vec<T> {
    fn merge( mut self, value: T ) -> Self {
        self.push( value );
        self
    }
    fn merge_all( mut self, values: impl IntoIterator<Item = T> ) -> Self {
        self.extend( values );
        self
    }
}

use std::collections::HashSet ;
use std::hash::Hash ;
impl<T: Eq + Hash> Merge<T> for HashSet<T> {
    fn merge( mut self, value: T ) -> Self {
        self.insert( value );
        self
    }
    fn merge_all( mut self, values: impl IntoIterator<Item = T> ) -> Self {
        values.into_iter().for_each(| value | { self.insert( value ); });
        self
    }
}
