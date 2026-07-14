use std::collections::HashMap ;

use crate::cardinality::{ Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne };
use crate::{ Val, nem };



#[test]
fn exactly_one_maps_and_gets() {
	let value = ExactlyOne( "plugin".to_string(), 10_u32 );
	let mapped = value.map(| id, value | format!( "{id}:{value}" ));
	assert_eq!( mapped.0, "plugin" );
	assert_eq!( mapped.1, "plugin:10" );
	assert_eq!( mapped.get( &"plugin".to_string() ), Some( &"plugin:10".to_string() ));
}

#[test]
#[should_panic( expected = "singleton cardinality id mismatch" )]
fn exactly_one_rejects_a_mismatched_id_in_debug_builds() {
	let value = ExactlyOne( "plugin".to_string(), 10_u32 );
	let _ = value.get( &"other".to_string() );
}

#[test]
fn at_most_one_maps_none_and_some() {
	let none: AtMostOne<String, u32> = AtMostOne( None );
	let mapped_none = none.map(| _, value | value + 1 );
	assert!( mapped_none.0.is_none() );
	assert_eq!( mapped_none.get( &"plugin".to_string() ), None );

	let none: AtMostOne<String, u32> = AtMostOne( None );
	assert!( none.map_mut(| value | value + 1 ).0.is_none() );

	let some = AtMostOne( Some(( "plugin".to_string(), 3_u32 )));
	let mapped_some = some.map(| _, value | value + 1 );
	assert_eq!( mapped_some.0, Some(( "plugin".to_string(), 4 )));
	assert_eq!( mapped_some.get( &"plugin".to_string() ), Some( &4 ));

	let some = AtMostOne( Some(( "plugin".to_string(), 3_u32 )));
	assert_eq!( some.map_mut(| value | value + 1 ).0, Some(( "plugin".to_string(), 4 )));
}

#[test]
#[should_panic( expected = "singleton cardinality id mismatch" )]
fn at_most_one_rejects_a_mismatched_id_in_debug_builds() {
	let value = AtMostOne( Some(( "plugin".to_string(), 10_u32 )));
	let _ = value.get( &"other".to_string() );
}

#[test]
fn at_least_one_maps_and_gets() {
	let values = AtLeastOne( nem! { "a".to_string() => 1_u32, "b".to_string() => 2_u32 } );
	let mapped = values.map(| _, value | value * 2 );
	assert_eq!( mapped.get( &"a".to_string() ), Some( &2 ));
	assert_eq!( mapped.get( &"b".to_string() ), Some( &4 ));

	let values = AtLeastOne( nem! { "a".to_string() => 1_u32, "b".to_string() => 2_u32 } );
	let mapped = values.map_mut(| value | value * 2 );
	assert_eq!( mapped.get( &"a".to_string() ), Some( &2 ));
	assert_eq!( mapped.get( &"b".to_string() ), Some( &4 ));
}

#[test]
fn any_maps_and_gets() {
	let values = Any( HashMap::from([
		( "a".to_string(), 1_u32 ),
		( "b".to_string(), 2_u32 ),
	]));
	let mapped = values.map(| _, value | value + 10 );
	assert_eq!( mapped.get( &"a".to_string() ), Some( &11 ));
	assert_eq!( mapped.get( &"b".to_string() ), Some( &12 ));
}

#[test]
fn exactly_one_into_val() {
	let val = Val::from( ExactlyOne( "id".to_string(), Val::U32( 7 )));
	assert!( matches!( val, Val::Tuple( items ) if
		items.len() == 2
		&& matches!( &items[0], Val::String( id ) if id == "id" )
		&& matches!( &items[1], Val::U32( 7 ))
	));
}

#[test]
fn at_most_one_into_val() {
	let none = Val::from( AtMostOne::<String, Val>( None ));
	assert!( matches!( none, Val::Option( None )));

	let some = Val::from( AtMostOne( Some(( "id".to_string(), Val::U32( 1 )))));
	assert!( matches!( some, Val::Option( Some( boxed )) if
		matches!( &*boxed, Val::Tuple( items ) if
			items.len() == 2
			&& matches!( &items[0], Val::String( id ) if id == "id" )
			&& matches!( &items[1], Val::U32( 1 ))
		)
	));
}

#[test]
fn at_least_one_into_val() {
	let val = Val::from( AtLeastOne( nem! { "a".to_string() => Val::U32( 1 ) }));
	assert!( matches!( val, Val::Map( items ) if
		items.len() == 1
		&& matches!( &items[0], ( Val::String( id ), Val::U32( 1 )) if id == "a" )
	));
}

#[test]
fn any_into_val() {
	let val = Val::from( Any( HashMap::from([
		( "a".to_string(), Val::U32( 1 )),
		( "b".to_string(), Val::U32( 2 )),
	])));
	assert!( matches!( val, Val::Map( items ) if
		items.len() == 2
		&& items.iter().any(|( key, value )|
			matches!( ( key, value ), ( Val::String( id ), Val::U32( 1 )) if id == "a" )
		)
		&& items.iter().any(|( key, value )|
			matches!( ( key, value ), ( Val::String( id ), Val::U32( 2 )) if id == "b" )
		)
	));
}
