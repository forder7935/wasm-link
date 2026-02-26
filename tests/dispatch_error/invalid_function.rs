use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, DispatchError, ExactlyOne };

fixtures! {
	bindings	= [ root: "root" ];
	plugins		= [ test_plugin: "test-plugin" ];
}

#[test]
fn dispatch_error_invalid_function() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.test_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "nonexistent-function", &[] ) {
		Err( DispatchError::InvalidFunction( _ )) => {}
		value => panic!( "Expected InvalidFunction error, found: {:#?}", value ),
	}

}
