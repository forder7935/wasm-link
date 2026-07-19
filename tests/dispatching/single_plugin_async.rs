use std::collections::HashMap;
use wasm_link::cardinality::ExactlyOne;
use wasm_link::concurrent::Binding;
use wasm_link::{Engine, Linker, Val};

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
		let engine = Engine::default();
		let linker = Linker::new( &engine );
        let executor =
            futures::executor::ThreadPool::new().expect("Failed to create async executor");
        let plugins = fixtures::plugins_concurrent(&engine);
        let bindings = fixtures::bindings_concurrent();

        let instance = plugins
            .plugin
            .plugin
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
	});
}
