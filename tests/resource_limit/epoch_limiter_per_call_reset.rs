use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, AtomicUsize, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind, Val, DispatchError };
use wasm_link::cardinality::ExactlyOne ;
use wasmtime::Config;

fixtures! {
	bindings = { root: "root" };
	plugins  = { burn_fuel: "burn-fuel" };
}

#[test]
fn closure_is_called_per_dispatch_and_deadline_is_reset() {

	let mut config = Config::new();
	config.epoch_interruption( true );
	let engine = Engine::new( &config ).expect( "failed to create engine" );
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let call_count = Arc::new( AtomicUsize::new( 0 ));
	let call_count_clone = Arc::clone( &call_count );

	let dispatch_call_count = Arc::new( AtomicUsize::new( 0 ));
	let dispatch_call_count_clone = Arc::clone( &dispatch_call_count );

	// First call returns a high deadline; subsequent calls return 1 (immediate exhaustion with a ticker).
	// The closure is not reset between dispatches.
	let plugin_instance = plugins.burn_fuel.plugin
		.with_epoch_limiter( move | _store, _interface, _function, _metadata | {
			dispatch_call_count_clone.fetch_add( 1, Ordering::Relaxed );
			if call_count_clone.fetch_add( 1, Ordering::Relaxed ) == 0 { 1_000_000 } else { 1 }
		})
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

	// First call: high deadline, no ticker -> success
	dispatch_call_count.store( 0, Ordering::Relaxed );
	match binding.dispatch( "root", "burn", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		other => panic!( "Expected Ok( U32( 42 )) on first dispatch, got: {:#?}", other ),
	}
	assert_eq!( dispatch_call_count.load( Ordering::Relaxed ), 1, "limiter should be called exactly once per dispatch" );

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

	// Second call: same closure now returns deadline 1, ticker running -> RuntimeException.
	// If the first call's deadline (1,000,000) carried over, this would succeed despite the ticker.
	dispatch_call_count.store( 0, Ordering::Relaxed );
	let result = binding.dispatch( "root", "burn", &[] );

	stop.store( true, Ordering::Release );
	let _ = handle.join();

	assert_eq!( dispatch_call_count.load( Ordering::Relaxed ), 1, "limiter should be called exactly once per dispatch" );
	match result {
		Ok( ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
		other => panic!( "Expected RuntimeException on second dispatch, got: {:#?}", other ),
	}
}
