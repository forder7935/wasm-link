use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings	= [ root: "root" ];
	plugins		= [ counter: "counter" ];
}

#[test]
fn resource_test_method_call() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.counter.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin_instance ),
	);

	let resource_handle = match binding.dispatch( "root", "[constructor]counter", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::Resource( handle )))) => handle,
		Ok( ExactlyOne( _, Ok( val ))) => panic!( "Expected resource, got: {:#?}", val ),
		Ok( ExactlyOne( _, Err( err ))) => panic!( "Constructor failed: {:?}", err ),
		value => panic!( "Expected Ok( ExactlyOne( Ok( Resource ))), got: {:#?}", value ),
	};

	match binding.dispatch( "root", "[method]counter.get-value", &[Val::Resource( resource_handle )] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		Ok( ExactlyOne( _, Ok( val ))) => panic!( "Expected U32(42), got: {:#?}", val ),
		Ok( ExactlyOne( _, Err( err ))) => panic!( "Method call failed: {:?}", err ),
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), got: {:#?}", value ),
	}

}
