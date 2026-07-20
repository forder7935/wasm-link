use std::collections::HashMap;
use wasm_link::{ concurrent::Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { plugin: "plugin" };
}

#[test]
fn synchronous_runtime_rejects_wit_async_components() {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let result = fixtures::plugins( &engine ).plugin.plugin.instantiate( &engine, &linker );
	let Err( error ) = result else { panic!( "WIT-async component instantiated synchronously" )};
	assert!( error.to_string().contains( "synchronous plugins cannot contain WIT-async functions" ));
}

#[test]
fn instantiates_and_dispatches_wit_async_plugin() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()
			.expect( "Failed to create async executor" );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();

		let instance = plugins.plugin.plugin
			.instantiate( &engine, &linker, executor )
			.await
			.expect( "Failed to instantiate plugin asynchronously" );
		let binding = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), instance ),
		);

		match binding.dispatch( "root", "get-value", &[] ).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
			value => panic!( "Expected async dispatch to return U32(42), found: {:#?}", value ),
		}
	});
}
