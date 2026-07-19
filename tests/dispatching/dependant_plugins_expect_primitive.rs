use std::collections::HashMap;
use std::sync::{ Arc, Barrier };
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn concurrent_sync_plugins_wait_for_their_shared_dependency() {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins_a = fixtures::plugins( &engine );
	let plugins_b = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let child = plugins_a.child.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate shared child plugin" );
	let dependency = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "child".to_string(), child ),
	);
	let startup_a = plugins_a.startup.plugin
		.link( &engine, linker.clone(), vec![ dependency.clone() ])
		.expect( "Failed to link first startup plugin" );
	let startup_b = plugins_b.startup.plugin
		.link( &engine, linker, vec![ dependency ])
		.expect( "Failed to link second startup plugin" );
	let root_a = Arc::new( Binding::new(
		bindings.root.package.clone(),
		HashMap::from([( bindings.root.name.clone(), bindings.root.spec.clone() )]),
		ExactlyOne( "startup-a".to_string(), startup_a ),
	));
	let root_b = Arc::new( Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "startup-b".to_string(), startup_b ),
	));
	let start = Arc::new( Barrier::new( 3 ));
	let calls = [ root_a, root_b ].map(| root | {
		let start = Arc::clone( &start );
		std::thread::spawn( move || {
			start.wait();
			root.dispatch( "root", "get-primitive", &[] )
		})
	});
	start.wait();

	calls.into_iter().for_each(| call | match call.join().expect( "dispatch thread panicked" ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "shared dependency call failed: {value:#?}" ),
	});
}

#[test]
fn dispatch_test_dependant_plugins_expect_primitive() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let child_instance = plugins.child.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate child plugin" );
	let dependency_binding = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "_".to_string(), child_instance ),
	);

	let startup_instance = plugins.startup.plugin
		.link( &engine, linker.clone(), vec![ dependency_binding ])
		.expect( "Failed to link startup plugin" );
	let root_binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), startup_instance ),
	);

	match root_binding.dispatch( "root", "get-primitive", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
	}

}

#[test]
fn dispatch_async_test_dependant_plugins_expect_primitive() {

	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()
			.expect( "Failed to create async executor" );
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();

		let child_instance = plugins.child.plugin
			.instantiate_async( &engine, &linker, executor.clone() )
			.await
			.expect( "Failed to instantiate child plugin asynchronously" );
		let dependency_binding = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child_instance ),
		);

		let startup_instance = plugins.startup.plugin
			.link_async( &engine, linker.clone(), vec![ dependency_binding ], executor )
			.await
			.expect( "Failed to link startup plugin asynchronously" );
		let root_binding = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup_instance ),
		);

		match root_binding.dispatch_async( "root", "get-primitive", &[] ).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
			value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
		}
	});

}
