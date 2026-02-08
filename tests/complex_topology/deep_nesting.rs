use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "level-b", "level-c" ];
	plugins		= [ "plugin-a", "plugin-b", "plugin-c" ];
}

#[test]
fn complex_topology_deep_nesting() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_c_instance = fixtures::plugin( "plugin-c", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin-c" );
	let interface_c = fixtures::interface( "level-c" );
	let binding_c = Binding::new(
		interface_c.package,
		HashMap::from([( interface_c.name, interface_c.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_c_instance ),
	);

	let plugin_b_instance = fixtures::plugin( "plugin-b", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_c ])
		.expect( "Failed to link plugin-b" );
	let interface_b = fixtures::interface( "level-b" );
	let binding_b = Binding::new(
		interface_b.package,
		HashMap::from([( interface_b.name, interface_b.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_b_instance ),
	);

	let plugin_a_instance = fixtures::plugin( "plugin-a", &engine ).plugin
		.link( &engine, linker.clone(), vec![ binding_b ])
		.expect( "Failed to link plugin-a" );
	let interface_root = fixtures::interface( "root" );
	let binding_root = Binding::new(
		interface_root.package,
		HashMap::from([( interface_root.name, interface_root.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_a_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 1 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 1 )))), found: {:#?}", value ),
	}

}
