use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	bindings	= [ root: "root" ];
	plugins		= [ get_composite: "get-composite" ];
}

#[test]
fn dispatch_test_single_plugin_expect_composite() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.get_composite.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "get-composite", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::Tuple( fields )))) => {
			assert_eq!( fields[0], Val::U32( 42 ));
			assert_eq!( fields[1], Val::U32( 24 ));
		}
		value => panic!( "Expected Ok( ExactlyOne( Ok( Tuple( ... )))), found: {:#?}", value ),
	}

}
