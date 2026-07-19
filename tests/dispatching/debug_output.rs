use std::collections::HashMap ;

use wasm_link::cardinality::ExactlyOne;
use wasm_link::concurrent::{
    Binding as ConcurrentBinding, PluginInstance as ConcurrentPluginInstance,
};
use wasm_link::{Binding, Engine, Linker, PluginInstance};

use crate::fixture_linking::TestContext ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { plugin: "plugin" };
}

#[test]
fn debug_output_exposes_configuration_without_component_internals() -> Result<(), wasmtime::Error> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let plugin_debug = format!( "{:?}", plugins.plugin.plugin );
	assert!( plugin_debug.contains( "component: \"<Component>\"" ));
	assert!( plugin_debug.contains( "initial_fuel: None" ));
	assert!( plugin_debug.contains( "fuel_limiter: None" ));

	let plugin_instance = plugins.plugin.plugin.instantiate( &engine, &linker )?;
	let instance_debug = format!( "{plugin_instance:?}" );
	assert!( instance_debug.contains( "data: TestContext" ));
	assert!( instance_debug.contains( "fuel_limiter: None" ));
    let binding: Binding<String, TestContext, ExactlyOne<String, PluginInstance<TestContext>>> =
        Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "plugin".to_string(), plugin_instance ),
	);
	let binding_debug = format!( "{binding:?}" );
	assert!( binding_debug.contains( "package_name: \"test:primitive\"" ));
	assert!( binding_debug.contains( "plugins: ExactlyOne" ));
	Ok(())
}

#[test]
fn async_debug_output_exposes_configuration_without_component_internals() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()?;
        let plugins = fixtures::plugins_concurrent(&engine);
        let bindings = fixtures::bindings_concurrent();
        let plugin_debug = format!("{:?}", plugins.plugin.plugin);
        assert!(plugin_debug.contains("component: \"<Component>\""));
        assert!(plugin_debug.contains("initial_fuel: None"));
        let plugin_instance = plugins
            .plugin
            .plugin
            .instantiate(&engine, &linker, executor)
            .await?;
		let instance_debug = format!( "{plugin_instance:?}" );
		assert!( instance_debug.contains( "state: \"<serialized store>\"" ));
		assert!( instance_debug.contains( "executor: \"<executor>\"" ));
        let binding: ConcurrentBinding<
			String,
			TestContext,
            ExactlyOne<String, ConcurrentPluginInstance<TestContext>>,
        > = ConcurrentBinding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "plugin".to_string(), plugin_instance ),
		);
		let binding_debug = format!( "{binding:?}" );
		assert!( binding_debug.contains( "package_name: \"test:primitive\"" ));
		assert!( binding_debug.contains( "plugins: ExactlyOne" ));
		Ok(())
	})
}
