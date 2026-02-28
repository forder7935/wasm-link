use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings	= [ root: "root", level_b: "level-b", level_c: "level-c" ];
	plugins		= [ plugin_a: "plugin-a", plugin_b: "plugin-b", plugin_c: "plugin-c" ];
}

#[test]
fn complex_topology_deep_nesting() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_c_instance = plugins.plugin_c.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin-c" );
	let binding_c = Binding::new(
		bindings.level_c.package,
		HashMap::from([( bindings.level_c.name, bindings.level_c.spec )]),
		ExactlyOne( "_".to_string(), plugin_c_instance ),
	);

	let plugin_b_instance = plugins.plugin_b.plugin
		.link( &engine, linker.clone(), vec![ binding_c ])
		.expect( "Failed to link plugin-b" );
	let binding_b = Binding::new(
		bindings.level_b.package,
		HashMap::from([( bindings.level_b.name, bindings.level_b.spec )]),
		ExactlyOne( "_".to_string(), plugin_b_instance ),
	);

	let plugin_a_instance = plugins.plugin_a.plugin
		.link( &engine, linker.clone(), vec![ binding_b ])
		.expect( "Failed to link plugin-a" );
	let binding_root = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin_a_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 1 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 1 )))), found: {:#?}", value ),
	}

}
