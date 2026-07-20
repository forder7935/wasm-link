use std::collections::HashMap;
use wasm_link::{ concurrent::Binding, Engine, FunctionKind, Linker, ReturnKind, Val };
use wasm_link::cardinality::ExactlyOne ;
use wasmtime::Config;

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
		let mut config = Config::new();
		config.consume_fuel( true ).epoch_interruption( true );
		let engine = Engine::new( &config ).expect( "Failed to create engine" );
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()
			.expect( "Failed to create async executor" );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();

		let instance = plugins.plugin.plugin
			.with_fuel_limiter(| _store, interface, name, function | {
				assert_eq!( interface, "test:single-async/root" );
				assert_eq!( name, "get-value" );
				assert_eq!( function.kind(), FunctionKind::Freestanding );
				assert_eq!( function.return_kind(), ReturnKind::AssumeNoResources );
				assert!( function.is_async() );
				100_000
			})
			.with_epoch_limiter(| _store, _, _, _ | 1_000_000 )
			.with_memory_limiter(| context | &mut context.store_limits )
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
