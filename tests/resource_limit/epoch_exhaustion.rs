use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind, ExactlyOne, Val };
use wasmtime::Config;

fixtures! {
	bindings    = [ root: "root" ];
	plugins     = [ burn_fuel: "burn-fuel" ];
}

fn dispatch_with_epoch( deadline: u64, concurrent_ticker: bool ) -> Result<ExactlyOne<String, Result<Val, wasm_link::DispatchError>>, wasm_link::DispatchError> {

	let mut config = Config::new();
	config.epoch_interruption( true );
	let engine = Engine::new( &config ).expect( "failed to create engine" );
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin_instance = plugins.burn_fuel.plugin
	.with_epoch_limiter( move | _store, _interface, _function, _metadata | deadline )
	.instantiate( &engine, &linker )
	.expect( "failed to instantiate plugin" );

	let binding = Binding::new(
	bindings.root.package,
	HashMap::from([( bindings.root.name, Interface::new(
		HashMap::from([( "burn".into(), Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ))]),
		HashSet::new(),
	))]),
	ExactlyOne( "_".to_string(), plugin_instance ),
	);

	if concurrent_ticker {
	let stop = Arc::new( AtomicBool::new( false ));
	let started = Arc::new( AtomicBool::new( false ));
	let stop_clone = Arc::clone( &stop );
	let started_clone = Arc::clone( &started );
	let engine_clone = engine.clone();
	let handle = thread::spawn( move || {
		while !stop_clone.load( Ordering::Acquire ) {
		engine_clone.increment_epoch();
		started_clone.store( true, Ordering::Release );
		thread::yield_now();
		}
	});
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs( 5 );
	while !started.load( Ordering::Acquire ) {
		assert!( std::time::Instant::now() < deadline, "ticker thread did not start in time" );
		thread::yield_now();
	}

	let result = binding.dispatch( "root", "burn", &[] );

	stop.store( true, Ordering::Release );
	let _ = handle.join();
	result
	} else {
	binding.dispatch( "root", "burn", &[] )
	}
}

#[test]
fn epoch_exhaustion_returns_runtime_exception() {
	// Deadline of 1 with concurrent ticker -> should trap after just 1 increment
	match dispatch_with_epoch( 1, true ) {
	Ok( ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
	other => panic!( "Expected RuntimeException from epoch exhaustion, got: {:#?}", other ),
	}
}

#[test]
fn sufficient_epoch_allows_completion() {
	// High deadline without ticker -> completion
	match dispatch_with_epoch( 1_000_000, false ) {
	Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
	other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
	}
}
