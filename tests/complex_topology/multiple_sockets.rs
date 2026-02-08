use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "dependency-one", "dependency-two", "dependency-three" ];
	plugins		= [ "root-plugin", "dependency-one-plugin", "dependency-two-plugin", "dependency-three-plugin" ];
}

#[test]
fn complex_topology_multiple_sockets() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let dependency_one_instance = fixtures::plugin( "dependency-one-plugin", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-one-plugin" );
	let interface_one = fixtures::interface( "dependency-one" );
	let binding_one = Binding::new(
		interface_one.package,
		HashMap::from([( interface_one.name, interface_one.interface )]),
		Socket::ExactlyOne( "_".to_string(), dependency_one_instance ),
	);

	let dependency_two_instance = fixtures::plugin( "dependency-two-plugin", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-two-plugin" );
	let interface_two = fixtures::interface( "dependency-two" );
	let binding_two = Binding::new(
		interface_two.package,
		HashMap::from([( interface_two.name, interface_two.interface )]),
		Socket::ExactlyOne( "_".to_string(), dependency_two_instance ),
	);

	let dependency_three_instance = fixtures::plugin( "dependency-three-plugin", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-three-plugin" );
	let interface_three = fixtures::interface( "dependency-three" );
	let binding_three = Binding::new(
		interface_three.package,
		HashMap::from([( interface_three.name, interface_three.interface )]),
		Socket::ExactlyOne( "_".to_string(), dependency_three_instance ),
	);

	let root_plugin_instance = fixtures::plugin( "root-plugin", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_one, binding_two, binding_three ])
		.expect( "Failed to link root-plugin" );
	let interface_root = fixtures::interface( "root" );
	let binding_root = Binding::new(
		interface_root.package,
		HashMap::from([( interface_root.name, interface_root.interface )]),
		Socket::ExactlyOne( "_".to_string(), root_plugin_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 6 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 6 )))), found: {:#?}", value ),
	}

}
