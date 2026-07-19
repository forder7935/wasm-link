use std::collections::HashMap;
use std::sync::{ Arc, mpsc };
use std::time::Duration ;
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;
use wasmtime::Config ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn concurrent_sync_plugins_wait_for_their_shared_dependency() {
	let mut config = Config::new();
	config.consume_fuel( true );
	let engine = Engine::new( &config ).expect( "Failed to create fuel-enabled engine" );
	let linker = Linker::new( &engine );
	let plugins_a = fixtures::plugins( &engine );
	let plugins_b = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let ( dependency_entered, entered ) = mpsc::channel();
	let ( release_dependency, release ) = mpsc::channel();
	let mut first_call = true;
	let child = plugins_a.child.plugin
		.with_initial_fuel( u64::MAX )
		.with_fuel_limiter( move | _, _, _, _ | {
			if first_call {
				first_call = false;
				dependency_entered.send(()).expect( "Failed to signal dependency entry" );
				release.recv().expect( "Failed to wait for dependency release" );
			}
			u64::MAX
		})
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate shared child plugin" );
	let dependency = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "child".to_string(), child ),
	);
	let startup_a = plugins_a.startup.plugin
		.with_initial_fuel( u64::MAX )
		.link( &engine, linker.clone(), vec![ dependency.clone() ])
		.expect( "Failed to link first startup plugin" );
	let ( second_plugin_entered, second_entered ) = mpsc::channel();
	let startup_b = plugins_b.startup.plugin
		.with_initial_fuel( u64::MAX )
		.with_fuel_limiter( move | _, _, _, _ | {
			second_plugin_entered.send(()).expect( "Failed to signal second plugin entry" );
			u64::MAX
		})
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
	let ( completed, results ) = mpsc::channel();
	let first_completed = completed.clone();
	let first = std::thread::spawn( move || first_completed.send(
		root_a.dispatch( "root", "get-primitive", &[] )
	).expect( "Failed to send first dispatch result" ));
	entered.recv_timeout( Duration::from_secs( 1 ))
		.expect( "First plugin did not enter the shared dependency" );
	let second = std::thread::spawn( move || completed.send(
		root_b.dispatch( "root", "get-primitive", &[] )
	).expect( "Failed to send second dispatch result" ));
	second_entered.recv_timeout( Duration::from_secs( 1 ))
		.expect( "Second plugin did not begin dispatch" );
	assert!( matches!(
		results.recv_timeout( Duration::from_millis( 50 )),
		Err( mpsc::RecvTimeoutError::Timeout ),
	), "A call completed while the shared dependency was deliberately held" );
	release_dependency.send(()).expect( "Failed to release shared dependency" );

	( 0..2 ).for_each(| _ | match results.recv_timeout( Duration::from_secs( 1 ))
		.expect( "A shared dependency call did not complete" )
	{
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "shared dependency call failed: {value:#?}" ),
	});
	first.join().expect( "First dispatch thread panicked" );
	second.join().expect( "Second dispatch thread panicked" );
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
