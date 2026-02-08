use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "get-composite" ];
}

#[test]
fn dispatch_test_single_plugin_expect_composite() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_instance = fixtures::plugin( "get-composite", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let interface = fixtures::interface( "root" );
	let binding = Binding::new(
		interface.package,
		HashMap::from([( interface.name, interface.interface )]),
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
