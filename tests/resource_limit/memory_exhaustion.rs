use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Component, Engine, Function, FunctionKind, Interface, Linker, Plugin, PluginContext, ResourceTable, ReturnKind, Val };
use wasm_link::cardinality::ExactlyOne ;

// Used to load the grow-memory WAT directly (fixtures::plugins() would return Plugin<TestContext>,
// but this test requires a custom context that holds the ResourceLimiter).
const FIXTURES_DIR: &str = "tests/resource_limit/memory_exhaustion";

fixtures! {
	bindings    = [ root: "root" ];
	plugins     = [];
}

struct TestCtx {
	resource_table: ResourceTable,
	limiter: MemoryLimiter,
}

impl PluginContext for TestCtx {
	fn resource_table( &mut self ) -> &mut ResourceTable {
		&mut self.resource_table
	}
}

struct MemoryLimiter {
	max_bytes: usize,
}

impl wasmtime::ResourceLimiter for MemoryLimiter {
	fn memory_growing( &mut self, _current: usize, desired: usize, _maximum: Option<usize> ) -> wasmtime::Result<bool> {
		Ok( desired <= self.max_bytes )
	}
	fn table_growing( &mut self, _current: usize, _desired: usize, _maximum: Option<usize> ) -> wasmtime::Result<bool> {
		Ok( true )
	}
}

fn dispatch_grow_memory( max_pages: usize ) -> Result<ExactlyOne<String, Result<Val, wasm_link::DispatchError>>, wasm_link::DispatchError> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let bindings = fixtures::bindings();

	let component = Component::from_file(
		&engine,
		format!( "{}/plugins/grow-memory/root.wat", FIXTURES_DIR ),
	).expect( "failed to load component" );

	let ctx = TestCtx {
		resource_table: ResourceTable::new(),
		limiter: MemoryLimiter { max_bytes: max_pages * 65536 },
	};

	let plugin_instance = Plugin::new( component, ctx )
		.with_memory_limiter( | ctx | &mut ctx.limiter )
		.instantiate( &engine, &linker )
		.expect( "failed to instantiate plugin" );

	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, Interface::new(
			HashMap::from([( "grow-memory".into(), Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ))]),
			HashSet::new(),
		))]),
		ExactlyOne( "_".to_string(), plugin_instance ),
	);

	binding.dispatch( "root", "grow-memory", &[] )
}

#[test]
fn tight_limit_denies_memory_growth() {
	match dispatch_grow_memory( 1 ) {
		Ok( ExactlyOne( _, Ok( Val::S32( -1 )))) => {}
		other => panic!( "Expected Ok( S32( -1 )) from denied memory growth, got: {:#?}", other ),
	}
}

#[test]
fn generous_limit_allows_memory_growth() {
	match dispatch_grow_memory( 2 ) {
		Ok( ExactlyOne( _, Ok( Val::S32( 1 )))) => {}
		other => panic!( "Expected Ok( S32( 1 )) from memory growth within limit, got: {:#?}", other ),
	}
}
