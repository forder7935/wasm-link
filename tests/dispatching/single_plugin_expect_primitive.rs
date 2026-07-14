use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { get_value: "get-value" };
}

#[test]
fn dispatch_test_single_plugin_expect_primitive() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let plugin_debug = format!( "{:?}", plugins.get_value.plugin );
	assert!( plugin_debug.contains( "component: \"<Component>\"" ));
	assert!( plugin_debug.contains( "fuel_limiter: None" ));

	let plugin_instance = plugins.get_value.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let instance_debug = format!( "{plugin_instance:?}" );
	assert!( instance_debug.contains( "data: TestContext" ));
	assert!( instance_debug.contains( "fuel_limiter: None" ));
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin_instance ),
	);
	let binding_debug = format!( "{binding:?}" );
	assert!( binding_debug.contains( "package_name: \"test:primitive\"" ));
	assert!( binding_debug.contains( "plugins: ExactlyOne" ));

	match binding.dispatch( "root", "get-primitive", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
	}

}
