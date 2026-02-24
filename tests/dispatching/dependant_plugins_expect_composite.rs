use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	bindings	= [ root: "root", dependency: "dependency" ];
	plugins		= [ startup: "startup", child: "child" ];
}

#[test]
fn dispatch_test_dependant_plugins_expect_composite() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let child_instance = plugins.child.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate child plugin" );
	let dependency_binding = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		Socket::ExactlyOne( "_".to_string(), child_instance ),
	);

	let startup_instance = plugins.startup.plugin
		.link( &engine, linker.clone(), vec![ dependency_binding ])
		.expect( "Failed to link startup plugin" );
	let root_binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
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
