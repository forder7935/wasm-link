use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "get-value" ];
}

#[test]
fn dispatch_test_single_plugin_expect_primitive() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_instance = fixtures::plugin( "get-value", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let interface = fixtures::interface( "root" );
	let binding = Binding::new(
		interface.package,
		HashMap::from([( interface.name, interface.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "get-primitive", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
	}

}
