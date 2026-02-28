//! Cardinality wrappers for plugin collections.

use std::collections::HashMap ;
use std::hash::Hash ;

use nonempty_collections::{ NEMap, NonEmptyIterator, IntoNonEmptyIterator };
use wasmtime::component::Val ;



/// Cardinality behavior for plugin containers.
///
/// Implementations preserve the container shape while allowing the inner value
/// type to be transformed.
pub trait Cardinality<Id, T>: Sized {
	/// Same cardinality with a different inner type.
	type Rebind<U>;

	/// Maps values by reference while preserving cardinality.
	fn map<N>( &self, map: impl FnMut( &Id, &T ) -> N ) -> Self::Rebind<N>
	where
		Id: Clone;

	/// Maps values by value while preserving cardinality.
	fn map_mut<N>( self, map: impl FnMut( T ) -> N ) -> Self::Rebind<N> ;

	/// Returns the value associated with `id`, if present.
	fn get( &self, id: &Id ) -> Option<&T>
	where
		Id: Hash + Eq ;
}

/// Exactly one value with ID, guaranteed present.
#[derive( Debug, Clone )]
pub struct ExactlyOne<Id, T>(
	/// The plugin identifier.
	pub Id,
	/// The associated value.
	pub T
);

/// Zero or one value with ID.
#[derive( Debug, Clone )]
pub struct AtMostOne<Id, T>(
	/// Optional `(id, value)` pair.
	pub Option<( Id, T )>
);

/// One or more values keyed by ID.
#[derive( Debug, Clone )]
pub struct AtLeastOne<Id, T>(
	/// Non-empty map keyed by plugin id.
	pub NEMap<Id, T>
);

/// Zero or more values keyed by ID.
#[derive( Debug, Clone )]
pub struct Any<Id, T>(
	/// Map keyed by plugin id.
	pub HashMap<Id, T>
);

impl<Id, T> Cardinality<Id, T> for ExactlyOne<Id, T> {
	type Rebind<U> = ExactlyOne<Id, U>;

	fn map<N>( &self, mut map: impl FnMut( &Id, &T ) -> N ) -> Self::Rebind<N>
	where
		Id: Clone,
	{
		ExactlyOne( self.0.clone(), map( &self.0, &self.1 ))
	}

	fn map_mut<N>( self, mut map: impl FnMut( T ) -> N ) -> Self::Rebind<N> {
		ExactlyOne( self.0, map( self.1 ))
	}

	fn get( &self, id: &Id ) -> Option<&T>
	where
		Id: Hash + Eq,
	{
		// In a singleton wrapper, mismatched ids indicate a logic bug upstream.
		// We still return the only value in release builds to avoid masking state.
		debug_assert!( &self.0 == id, "singleton cardinality id mismatch" );
		Some( &self.1 )
	}
}

impl<Id, T> Cardinality<Id, T> for AtMostOne<Id, T> {
	type Rebind<U> = AtMostOne<Id, U>;

	fn map<N>( &self, mut map: impl FnMut( &Id, &T ) -> N ) -> Self::Rebind<N>
	where
		Id: Clone,
	{
		match &self.0 {
			None => AtMostOne( None ),
			Some(( id, value )) => AtMostOne( Some(( id.clone(), map( id, value )))),
		}
	}

	fn map_mut<N>( self, mut map: impl FnMut( T ) -> N ) -> Self::Rebind<N> {
		match self.0 {
			None => AtMostOne( None ),
			Some(( id, value )) => AtMostOne( Some(( id, map( value )))),
		}
	}

	fn get( &self, id: &Id ) -> Option<&T>
	where
		Id: Hash + Eq,
	{
		match self.0.as_ref() {
			None => None,
			Some(( stored_id, value )) => {
				// In a singleton wrapper, mismatched ids indicate a logic bug upstream.
				// We still return the only value in release builds to avoid masking state.
				debug_assert!( stored_id == id, "singleton cardinality id mismatch" );
				Some( value )
			}
		}
	}
}

impl<Id: Hash + Eq, T> Cardinality<Id, T> for AtLeastOne<Id, T> {
	type Rebind<U> = AtLeastOne<Id, U>;

	fn map<N>( &self, mut map: impl FnMut( &Id, &T ) -> N ) -> Self::Rebind<N>
	where
		Id: Clone,
	{
		AtLeastOne(
			self.0.nonempty_iter()
				.map(|( id, value )| ( id.clone(), map( id, value )))
				.collect()
		)
	}

	fn map_mut<N>( self, mut map: impl FnMut( T ) -> N ) -> Self::Rebind<N> {
		AtLeastOne(
			self.0.into_nonempty_iter()
				.map(|( id, value )| ( id, map( value )))
				.collect()
		)
	}

	fn get( &self, id: &Id ) -> Option<&T>
	where
		Id: Hash + Eq,
	{
		self.0.get( id )
	}
}

impl<Id: Hash + Eq, T> Cardinality<Id, T> for Any<Id, T> {
	type Rebind<U> = Any<Id, U>;

	fn map<N>( &self, mut map: impl FnMut( &Id, &T ) -> N ) -> Self::Rebind<N>
	where
		Id: Clone,
	{
		Any( self.0.iter().map(|( id, value )| ( id.clone(), map( id, value ))).collect() )
	}

	fn map_mut<N>( self, mut map: impl FnMut( T ) -> N ) -> Self::Rebind<N> {
		Any( self.0.into_iter().map(|( id, value )| ( id, map( value ))).collect() )
	}

	fn get( &self, id: &Id ) -> Option<&T>
	where
		Id: Hash + Eq,
	{
		self.0.get( id )
	}
}

impl<Id: Hash + Eq + Into<Val>> From<ExactlyOne<Id, Val>> for Val {
	fn from( socket: ExactlyOne<Id, Val> ) -> Self {
		Val::Tuple( vec![ socket.0.into(), socket.1 ])
	}
}

impl<Id: Hash + Eq + Into<Val>> From<AtMostOne<Id, Val>> for Val {
	fn from( socket: AtMostOne<Id, Val> ) -> Self {
		match socket.0 {
			None => Val::Option( None ),
			Some(( id, val )) => Val::Option( Some( Box::new( Val::Tuple( vec![ id.into(), val ] )))),
		}
	}
}

impl<Id: Hash + Eq + Into<Val>> From<AtLeastOne<Id, Val>> for Val {
	fn from( socket: AtLeastOne<Id, Val> ) -> Self {
		Val::List(
			socket.0.into_iter()
				.map(|( id, val )| Val::Tuple( vec![ id.into(), val ]))
				.collect()
		)
	}
}

impl<Id: Hash + Eq + Into<Val>> From<Any<Id, Val>> for Val {
	fn from( socket: Any<Id, Val> ) -> Self {
		Val::List(
			socket.0.into_iter()
				.map(|( id, val )| Val::Tuple( vec![ id.into(), val ]))
				.collect()
		)
	}
}

#[cfg(test)]
mod tests {

	use crate::nem ;
	use super::* ;

	#[test]
	fn exactly_one_maps_and_gets() {
		let value = ExactlyOne( "plugin".to_string(), 10_u32 );
		let mapped = value.map(| id, v | format!( "{id}:{v}" ));
		assert_eq!( mapped.0, "plugin" );
		assert_eq!( mapped.1, "plugin:10" );
		assert_eq!( mapped.get( &"plugin".to_string() ), Some( &"plugin:10".to_string() ));
	}

	#[test]
	fn at_most_one_maps_none_and_some() {
		let none: AtMostOne<String, u32> = AtMostOne( None );
		let mapped_none = none.map(| _, v | v + 1 );
		assert!( mapped_none.0.is_none() );

		let some = AtMostOne( Some(( "plugin".to_string(), 3_u32 )));
		let mapped_some = some.map(| _, v | v + 1 );
		assert_eq!( mapped_some.0, Some(( "plugin".to_string(), 4 )));
	}

	#[test]
	fn at_least_one_maps_and_gets() {
		let values = AtLeastOne( nem! { "a".to_string() => 1_u32, "b".to_string() => 2_u32 } );
		let mapped = values.map(| _, v | v * 2 );
		assert_eq!( mapped.get( &"a".to_string() ), Some( &2 ));
		assert_eq!( mapped.get( &"b".to_string() ), Some( &4 ));
	}

	#[test]
	fn any_maps_and_gets() {
		let values = Any( HashMap::from([
			( "a".to_string(), 1_u32 ),
			( "b".to_string(), 2_u32 ),
		]));
		let mapped = values.map(| _, v | v + 10 );
		assert_eq!( mapped.get( &"a".to_string() ), Some( &11 ));
		assert_eq!( mapped.get( &"b".to_string() ), Some( &12 ));
	}

	#[test]
	fn exactly_one_into_val() {
		let val = Val::from( ExactlyOne( "id".to_string(), Val::U32( 7 )));
		match val {
			Val::Tuple( items ) => {
				assert_eq!( items.len(), 2 );
				assert!( matches!( &items[0], Val::String( s ) if s == "id" ));
				assert!( matches!( &items[1], Val::U32( 7 )));
			}
			other => panic!( "expected tuple, got {other:?}" ),
		}
	}

	#[test]
	fn at_most_one_into_val() {
		let none = Val::from( AtMostOne::<String, Val>( None ));
		assert!( matches!( none, Val::Option( None )));

		let some = Val::from( AtMostOne( Some(( "id".to_string(), Val::U32( 1 )))));
		match some {
			Val::Option( Some( boxed )) => match *boxed {
				Val::Tuple( items ) => {
					assert_eq!( items.len(), 2 );
					assert!( matches!( &items[0], Val::String( s ) if s == "id" ));
					assert!( matches!( &items[1], Val::U32( 1 )));
				}
				other => panic!( "expected tuple, got {other:?}" ),
			},
			other => panic!( "expected option, got {other:?}" ),
		}
	}

	#[test]
	fn at_least_one_into_val() {
		let val = Val::from( AtLeastOne( nem! { "a".to_string() => Val::U32( 1 ) }));
		match val {
			Val::List( items ) => {
				assert_eq!( items.len(), 1 );
				assert!( matches!( &items[0],
					Val::Tuple( tuple )
						if tuple.len() == 2
						&& matches!( &tuple[0], Val::String( s ) if s == "a" )
						&& matches!( &tuple[1], Val::U32( 1 ))
				));
			}
			other => panic!( "expected list, got {other:?}" ),
		}
	}

	#[test]
	fn any_into_val() {
		let val = Val::from( Any( HashMap::from([
			( "a".to_string(), Val::U32( 1 )),
			( "b".to_string(), Val::U32( 2 )),
		])));
		match val {
			Val::List( items ) => {
				assert_eq!( items.len(), 2 );
				let mut seen = ( false, false );
				for item in items {
					match item {
						Val::Tuple( tuple ) if tuple.len() == 2 => match (&tuple[0], &tuple[1]) {
							( Val::String( s ), Val::U32( 1 )) if s == "a" => seen.0 = true,
							( Val::String( s ), Val::U32( 2 )) if s == "b" => seen.1 = true,
							_ => {}
						},
						_ => {}
					}
				}
				assert!( seen.0 && seen.1 );
			}
			other => panic!( "expected list, got {other:?}" ),
		}
	}
}
