use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "counter" ];
}

#[test]
fn resource_test_method_call() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_instance = fixtures::plugin( "counter", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let interface = fixtures::interface( "root" );
	let binding = Binding::new(
		interface.package,
		HashMap::from([( interface.name, interface.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	let resource_handle = match binding.dispatch( "root", "[constructor]counter", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::Resource( handle )))) => handle,
		Ok( Socket::ExactlyOne( _, Ok( val ))) => panic!( "Expected resource, got: {:#?}", val ),
		Ok( Socket::ExactlyOne( _, Err( err ))) => panic!( "Constructor failed: {:?}", err ),
		value => panic!( "Expected Ok( ExactlyOne( Ok( Resource ))), got: {:#?}", value ),
	};

	match binding.dispatch( "root", "[method]counter.get-value", &[Val::Resource( resource_handle )] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		Ok( Socket::ExactlyOne( _, Ok( val ))) => panic!( "Expected U32(42), got: {:#?}", val ),
		Ok( Socket::ExactlyOne( _, Err( err ))) => panic!( "Method call failed: {:?}", err ),
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), got: {:#?}", value ),
	}

}
