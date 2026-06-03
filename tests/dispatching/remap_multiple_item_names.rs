use std::collections::HashMap ;
use wasm_link::{ Binding, Engine, Linker, Remap, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { root: "root" };
}

#[test]
fn dispatch_remaps_multiple_item_names() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin = plugins.root.plugin
		.remap_interfaces( HashMap::from([(
			"root".to_string(),
			Remap::item_resolution_table( HashMap::from([
				( "get-one".to_string(), "legacy-get-one".to_string() ),
				( "get-two".to_string(), "legacy-get-two".to_string() ),
			])),
		)]))
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );

	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin ),
	);

	match binding.dispatch( "root", "get-one", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 1 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 1 )))), found: {value:#?}" ),
	}

	match binding.dispatch( "root", "get-two", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 2 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 2 )))), found: {value:#?}" ),
	}

}
