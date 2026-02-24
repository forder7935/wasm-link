use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	bindings	= [ root: "root" ];
	plugins		= [ get_value: "get-value" ];
}

#[test]
fn dispatch_test_single_plugin_expect_primitive() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.get_value.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "get-primitive", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
	}

}
