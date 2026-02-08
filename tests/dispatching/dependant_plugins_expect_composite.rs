use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "dependency" ];
	plugins		= [ "startup", "child" ];
}

#[test]
fn dispatch_test_dependant_plugins_expect_composite() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let child_instance = fixtures::plugin( "child", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate child plugin" );
	let interface_dependency = fixtures::interface( "dependency" );
	let dependency_binding = Binding::new(
		interface_dependency.package,
		HashMap::from([( interface_dependency.name, interface_dependency.interface )]),
		Socket::ExactlyOne( "_".to_string(), child_instance ),
	);

	let startup_instance = fixtures::plugin( "startup", &engine ).plugin
		.link( &engine, linker.clone(), vec![ dependency_binding ])
		.expect( "Failed to link startup plugin" );
	let interface_root = fixtures::interface( "root" );
	let root_binding = Binding::new(
		interface_root.package,
		HashMap::from([( interface_root.name, interface_root.interface )]),
		Socket::ExactlyOne( "_".to_string(), startup_instance ),
	);

	match root_binding.dispatch( "root", "get-composite", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::Tuple( fields )))) => {
			assert_eq!( fields[0], Val::U32( 42 ));
			assert_eq!( fields[1], Val::U32( 24 ));
		}
		value => panic!( "Expected Ok( ExactlyOne( Ok( Tuple( ... )))), found: {:#?}", value ),
	}

}
