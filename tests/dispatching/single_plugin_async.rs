use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasm_link::cardinality::ExactlyOne;
use wasm_link::concurrent::Binding;
use wasm_link::{Engine, Linker, Val};
use wasmtime::Config;

fixtures! {
	bindings = { root: "root" };
	plugins  = { plugin: "plugin" };
}

#[test]
fn synchronous_runtime_rejects_a_wit_async_component() {
    let engine = Engine::default();
    let linker = Linker::new(&engine);
    let plugin = fixtures::plugins(&engine).plugin.plugin;
    let error = plugin
        .instantiate(&engine, &linker)
        .expect_err("a WIT-async component must not instantiate in the synchronous runtime");
    assert!(
        error.to_string().contains("async"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn instantiates_and_dispatches_wit_async_plugin() {
	futures::executor::block_on( async {
		let mut config = Config::new();
		config.consume_fuel(true);
		config.epoch_interruption(true);
		config.concurrency_support(true);
		let engine = Engine::new(&config).expect("Failed to create concurrent engine");
		let linker = Linker::new( &engine );
        let executor =
            futures::executor::ThreadPool::new().expect("Failed to create async executor");
        let plugins = fixtures::plugins_concurrent(&engine);
        let bindings = fixtures::bindings_concurrent();
		let fuel_kinds = Arc::new(Mutex::new(Vec::new()));
		let epoch_kinds = Arc::new(Mutex::new(Vec::new()));
		let observed_fuel_kinds = Arc::clone(&fuel_kinds);
		let observed_epoch_kinds = Arc::clone(&epoch_kinds);

        let instance = plugins
            .plugin
            .plugin
			.with_fuel_limiter(move |_, _, _, function| {
				observed_fuel_kinds.lock().expect("fuel observations poisoned").push(function.is_async());
				u64::MAX
			})
			.with_epoch_limiter(move |_, _, _, function| {
				observed_epoch_kinds.lock().expect("epoch observations poisoned").push(function.is_async());
				u64::MAX
			})
			.with_memory_limiter(|context| &mut context.limits)
            .instantiate(&engine, &linker, executor)
			.await
			.expect( "Failed to instantiate plugin asynchronously" );
		let binding = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), instance ),
		);

        match binding.dispatch("root", "get-value", &[]).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
			value => panic!( "Expected async dispatch to return U32(42), found: {:#?}", value ),
		}
		match binding.dispatch("root", "get-sync-value", &[]).await {
			Ok(ExactlyOne(_, Ok(Val::U32(42)))) => {}
			value => panic!("Expected sync dispatch to return U32(42), found: {value:#?}"),
		}
		assert_eq!(*fuel_kinds.lock().expect("fuel observations poisoned"), [true, false]);
		assert_eq!(*epoch_kinds.lock().expect("epoch observations poisoned"), [true, false]);
	});
}
