use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "interface-b", "interface-c", "interface-d" ];
	plugins		= [ "plugin-a", "plugin-b", "plugin-c", "plugin-d" ];
}

#[test]
fn complex_topology_shared_dependency() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_d_instance = fixtures::plugin( "plugin-d", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin-d" );
	let interface_d = fixtures::interface( "interface-d" );
	let binding_d = Binding::new(
		interface_d.package,
		HashMap::from([( interface_d.name, interface_d.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_d_instance ),
	);

	let plugin_b_instance = fixtures::plugin( "plugin-b", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_d.clone() ])
		.expect( "Failed to link plugin-b" );
	let interface_b = fixtures::interface( "interface-b" );
	let binding_b = Binding::new(
		interface_b.package,
		HashMap::from([( interface_b.name, interface_b.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_b_instance ),
	);

	let plugin_c_instance = fixtures::plugin( "plugin-c", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_d ])
		.expect( "Failed to link plugin-c" );
	let interface_c = fixtures::interface( "interface-c" );
	let binding_c = Binding::new(
		interface_c.package,
		HashMap::from([( interface_c.name, interface_c.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_c_instance ),
	);

	let plugin_a_instance = fixtures::plugin( "plugin-a", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_b, binding_c ])
		.expect( "Failed to link plugin-a" );
	let interface_root = fixtures::interface( "root" );
	let binding_root = Binding::new(
		interface_root.package,
		HashMap::from([( interface_root.name, interface_root.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_a_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 2 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 2 )))), found: {:#?}", value ),
	}

}
