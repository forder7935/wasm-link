use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, Socket };

fixtures! {
	bindings	= [ root: "root", binding_b: "binding-b", binding_c: "binding-c", binding_d: "binding-d" ];
	plugins		= [ plugin_a: "plugin-a", plugin_b: "plugin-b", plugin_c: "plugin-c", plugin_d: "plugin-d" ];
}

#[test]
fn complex_topology_shared_dependency() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_d_instance = plugins.plugin_d.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin-d" );
	let binding_d = Binding::new(
		bindings.binding_d.package,
		HashMap::from([( bindings.binding_d.name, bindings.binding_d.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_d_instance ),
	);

	let plugin_b_instance = plugins.plugin_b.plugin
		.link( &engine, linker.clone(), vec![ binding_d.clone() ])
		.expect( "Failed to link plugin-b" );
	let binding_b = Binding::new(
		bindings.binding_b.package,
		HashMap::from([( bindings.binding_b.name, bindings.binding_b.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_b_instance ),
	);

	let plugin_c_instance = plugins.plugin_c.plugin
		.link( &engine, linker.clone(), vec![ binding_d ])
		.expect( "Failed to link plugin-c" );
	let binding_c = Binding::new(
		bindings.binding_c.package,
		HashMap::from([( bindings.binding_c.name, bindings.binding_c.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_c_instance ),
	);

	let plugin_a_instance = plugins.plugin_a.plugin
		.link( &engine, linker.clone(), vec![ binding_b, binding_c ])
		.expect( "Failed to link plugin-a" );
	let binding_root = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Socket::ExactlyOne( "_".to_string(), plugin_a_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( Socket::ExactlyOne( _, Ok( Val::U32( 2 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 2 )))), found: {:#?}", value ),
	}

}
