use std::collections::{ HashMap, HashSet };
use wasm_link::{ sync::Binding, DispatchError, Engine, sync::Function, FunctionKind, sync::Interface, Linker, ReturnKind };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { test_plugin: "test-plugin" };
}

#[test]
fn async_dispatch_error_invalid_function() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();
		let plugin = plugins.test_plugin.plugin.instantiate(
			&engine,
			&linker,
			futures::executor::ThreadPool::new()?,
		).await?;
		let binding = wasm_link::concurrent::Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), plugin ),
		);

		assert!( matches!(
			binding.dispatch( "root", "nonexistent-function", &[] ).await,
			Err( DispatchError::InvalidFunction( _ ))
		));
		Ok(())
	})
}

#[test]
fn plugin_dispatch_rejects_a_non_function_export() -> Result<(), wasmtime::Error> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let plugin = plugins.test_plugin.plugin.instantiate( &engine, &linker )?;
	let binding = Binding::new(
		"test:dispatch-error",
		HashMap::from([(
			"root".to_string(),
			Interface::new(
				HashMap::from([(
					"not-a-function".to_string(),
					Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ),
				)]),
				HashSet::new(),
			),
		)]),
		ExactlyOne( "_".to_string(), plugin ),
	);

	assert!( matches!(
		binding.dispatch( "root", "not-a-function", &[] ),
		Ok( ExactlyOne( _, Err( DispatchError::InvalidFunction( _ ))))
	));
	Ok(())
}

#[test]
fn dispatch_error_invalid_function() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.test_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "nonexistent-function", &[] ) {
		Err( DispatchError::InvalidFunction( _ )) => {}
		value => panic!( "Expected InvalidFunction error, found: {:#?}", value ),
	}

}
