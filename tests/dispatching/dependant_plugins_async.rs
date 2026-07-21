use std::collections::HashMap;
use wasm_link::{ concurrent::Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

struct RejectingExecutor ;

impl futures::task::Spawn for RejectingExecutor {
	fn spawn_obj( &self, _future: futures::task::FutureObj<'static, ()> ) -> Result<(), futures::task::SpawnError> {
		Err( futures::task::SpawnError::shutdown() )
	}
}

#[test]
fn links_and_dispatches_wit_async_across_plugin_stores_on_one_worker() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::builder()
			.pool_size( 1 )
			.create()
			.expect( "Failed to create async executor" );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();

		let child_instance = plugins.child.plugin
			.instantiate( &engine, &linker, executor.clone() )
			.await
			.expect( "Failed to instantiate child plugin asynchronously" );
		let dependency_binding = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child_instance ),
		);

		let startup_instance = plugins.startup.plugin
			.link( &engine, linker, vec![ dependency_binding ], executor )
			.await
			.expect( "Failed to link startup plugin asynchronously" );
		let root_binding = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup_instance ),
		);

		match root_binding.dispatch( "root", "get-primitive", &[] ).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
			value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
		}

		let ( first, second ) = futures::join!(
			root_binding.dispatch( "root", "get-primitive", &[] ),
			root_binding.dispatch( "root", "get-primitive", &[] ),
		);
		for value in [ first, second ] {
			match value {
				Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
				value => panic!( "Expected queued async dispatch to return U32(42), found: {:#?}", value ),
			}
		}
	});
}

#[test]
fn reports_when_the_supplied_executor_rejects_dispatch() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();
		let child_instance = plugins.child.plugin
			.instantiate( &engine, &linker, RejectingExecutor )
			.await
			.expect( "Failed to instantiate child plugin asynchronously" );
		let binding = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child_instance ),
		);

		match binding.dispatch( "root", "get-value", &[] ).await {
			Ok( ExactlyOne( _, Err( wasm_link::DispatchError::ExecutorUnavailable ))) => {}
			value => panic!( "Expected ExecutorUnavailable, found: {:#?}", value ),
		}
	});
}

#[test]
fn propagates_executor_rejection_across_a_plugin_link() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()
			.expect( "Failed to create async executor" );
		let plugins = fixtures::concurrent_plugins( &engine );
		let bindings = fixtures::concurrent_bindings();
		let child = plugins.child.plugin
			.instantiate( &engine, &linker, RejectingExecutor )
			.await
			.expect( "Failed to instantiate child plugin asynchronously" );
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let startup = plugins.startup.plugin
			.link( &engine, linker, vec![ dependency ], executor )
			.await
			.expect( "Failed to link startup plugin asynchronously" );
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup ),
		);

		match root.dispatch( "root", "get-primitive", &[] ).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 0 )))) => {}
			value => panic!( "Expected the consumer's error fallback U32(0), found: {value:#?}" ),
		}
	});
}
