use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, DispatchError, Socket };

fixtures! {
	bindings	= [ root: "root" ];
	plugins		= [ test_plugin: "test-plugin" ];
}

#[test]
fn dispatch_error_wrong_argument_count() {

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
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "add", &[] ) {
		Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( err )))) if err.to_string().contains( "expected 2 argument" ) => {}
		value => panic!( "Expected RuntimeException about argument count, found: {:#?}", value ),
	}

}
