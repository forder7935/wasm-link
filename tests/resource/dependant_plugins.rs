use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "dependency" ];
	plugins		= [ "consumer", "counter" ];
}

#[test]
fn resource_test_wrapper() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let counter_instance = fixtures::plugin( "counter", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate counter plugin" );
	let interface_dependency = fixtures::interface( "dependency" );
	let dependency_binding = Binding::new(
		interface_dependency.package,
		HashMap::from([( interface_dependency.name, interface_dependency.interface )]),
		Socket::ExactlyOne( "_".to_string(), counter_instance ),
	);

	let consumer_instance = fixtures::plugin( "consumer", &engine ).plugin
		.link( &engine, linker.clone(), vec![ dependency_binding ])
		.expect( "Failed to link consumer plugin" );
	let interface_root = fixtures::interface( "root" );
	let root_binding = Binding::new(
		interface_root.package,
		HashMap::from([( interface_root.name, interface_root.interface )]),
		Socket::ExactlyOne( "_".to_string(), consumer_instance ),
	);

	match root_binding.dispatch( "root", "get-value", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		Ok( Socket::ExactlyOne( _, Ok( val ))) => panic!( "Expected U32(42), got: {:#?}", val ),
		Ok( Socket::ExactlyOne( _, Err( err ))) => panic!( "Method call failed: {:?}", err ),
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), got: {:#?}", value ),
	}

}
