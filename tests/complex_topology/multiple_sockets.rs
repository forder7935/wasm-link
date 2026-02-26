use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val, ExactlyOne };

fixtures! {
	bindings	= [ root: "root", dependency_one: "dependency-one", dependency_two: "dependency-two", dependency_three: "dependency-three" ];
	plugins		= [ root_plugin: "root-plugin", dependency_one_plugin: "dependency-one-plugin", dependency_two_plugin: "dependency-two-plugin", dependency_three_plugin: "dependency-three-plugin" ];
}

#[test]
fn complex_topology_multiple_sockets() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let dependency_one_instance = plugins.dependency_one_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-one-plugin" );
	let binding_one = Binding::new(
		bindings.dependency_one.package,
		HashMap::from([( bindings.dependency_one.name, bindings.dependency_one.spec )]),
		ExactlyOne( "_".to_string(), dependency_one_instance ),
	);

	let dependency_two_instance = plugins.dependency_two_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-two-plugin" );
	let binding_two = Binding::new(
		bindings.dependency_two.package,
		HashMap::from([( bindings.dependency_two.name, bindings.dependency_two.spec )]),
		ExactlyOne( "_".to_string(), dependency_two_instance ),
	);

	let dependency_three_instance = plugins.dependency_three_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate dependency-three-plugin" );
	let binding_three = Binding::new(
		bindings.dependency_three.package,
		HashMap::from([( bindings.dependency_three.name, bindings.dependency_three.spec )]),
		ExactlyOne( "_".to_string(), dependency_three_instance ),
	);

	let root_plugin_instance = plugins.root_plugin.plugin
		.link( &engine, linker.clone(), vec![ binding_one, binding_two, binding_three ])
		.expect( "Failed to link root-plugin" );
	let binding_root = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), root_plugin_instance ),
	);

	match binding_root.dispatch( "root", "get-value", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 6 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 6 )))), found: {:#?}", value ),
	}

}
