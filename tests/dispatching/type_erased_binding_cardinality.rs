use std::collections::HashMap ;

use wasm_link::{ Engine, Linker, nem };
use wasm_link::concurrent::{ Binding as ConcurrentBinding, PluginInstance as ConcurrentPluginInstance };
use wasm_link::sync::{ Binding, PluginInstance };
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

#[test]
fn links_each_async_type_erased_binding_cardinality() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let executor = futures::executor::ThreadPool::new()?;

		let bindings = fixtures::concurrent_bindings();
		let instance = fixtures::concurrent_plugins( &engine ).plugin.plugin
			.instantiate( &engine, &Linker::new( &engine ), executor.clone() ).await?;
		let binding: ConcurrentBinding<
			String,
			TestContext,
			ExactlyOne<String, ConcurrentPluginInstance<TestContext>>,
		> = ConcurrentBinding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);
		let socket = binding.into_any();
		let plugin = fixtures::concurrent_plugins( &engine ).plugin.plugin;
		let _ = plugin.link(
			&engine,
			Linker::new( &engine ),
			vec![ socket ],
			executor.clone(),
		).await?;

		let bindings = fixtures::concurrent_bindings();
		let binding: ConcurrentBinding<
			String,
			TestContext,
			AtMostOne<String, ConcurrentPluginInstance<TestContext>>,
		> = ConcurrentBinding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			AtMostOne( None ),
		);
		let socket = binding.into_any();
		let plugin = fixtures::concurrent_plugins( &engine ).plugin.plugin;
		let _ = plugin.link(
			&engine,
			Linker::new( &engine ),
			vec![ socket ],
			executor.clone(),
		).await?;

		let bindings = fixtures::concurrent_bindings();
		let instance = fixtures::concurrent_plugins( &engine ).plugin.plugin
			.instantiate( &engine, &Linker::new( &engine ), executor.clone() ).await?;
		let binding: ConcurrentBinding<
			String,
			TestContext,
			AtLeastOne<String, ConcurrentPluginInstance<TestContext>>,
		> = ConcurrentBinding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			AtLeastOne( nem! { "plugin".to_string() => instance }),
		);
		let socket = binding.into_any();
		let plugin = fixtures::concurrent_plugins( &engine ).plugin.plugin;
		let _ = plugin.link(
			&engine,
			Linker::new( &engine ),
			vec![ socket ],
			executor.clone(),
		).await?;

		let bindings = fixtures::concurrent_bindings();
		let binding: ConcurrentBinding<
			String,
			TestContext,
			Any<String, ConcurrentPluginInstance<TestContext>>,
		> = ConcurrentBinding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			Any( HashMap::new() ),
		);
		let socket = binding.into_any();
		let plugin = fixtures::concurrent_plugins( &engine ).plugin.plugin;
		let _ = plugin.link(
			&engine,
			Linker::new( &engine ),
			vec![ socket ],
			executor,
		).await?;

		Ok(())
	})
}
