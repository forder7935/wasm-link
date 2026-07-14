use std::collections::HashMap ;

use wasm_link::{ Binding, Engine, Linker, PluginInstance, nem };
use wasm_link::cardinality::{ Any, AtLeastOne, AtMostOne, ExactlyOne };

use crate::fixture_linking::TestContext ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { plugin: "plugin" };
}

#[test]
fn links_each_type_erased_binding_cardinality() -> Result<(), wasmtime::Error> {
	let engine = Engine::default();

	let bindings = fixtures::bindings();
	let instance = fixtures::plugins( &engine ).plugin.plugin
		.instantiate( &engine, &Linker::new( &engine ))?;
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "plugin".to_string(), instance ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).plugin.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let binding: Binding<String, TestContext, AtMostOne<String, PluginInstance<TestContext>>> = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		AtMostOne( None ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).plugin.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let instance = fixtures::plugins( &engine ).plugin.plugin
		.instantiate( &engine, &Linker::new( &engine ))?;
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		AtLeastOne( nem! { "plugin".to_string() => instance }),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).plugin.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let binding: Binding<String, TestContext, Any<String, PluginInstance<TestContext>>> = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Any( HashMap::new() ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).plugin.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;
	Ok(())
}
