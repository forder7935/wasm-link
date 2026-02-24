use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, AtomicU32, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

#[test]
fn closure_is_called_per_dispatch_and_deadline_is_reset() {

    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let call_count = Arc::new( AtomicU32::new( 0 ));
    let call_count_clone = Arc::clone( &call_count );

    // First call returns a high deadline; subsequent calls return 1 (immediate exhaustion with a ticker).
    // The closure is not reset between dispatches.
    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_epoch_limiter( move | _store, _interface, _function, _metadata | {
            if call_count_clone.fetch_add( 1, Ordering::Relaxed ) == 0 { 1_000_000 } else { 1 }
        })
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let binding = Binding::new(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    // First call: high deadline, no ticker -> success
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )) on first dispatch, got: {:#?}", other ),
    }

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
    while !started.load( Ordering::Acquire ) {
        thread::yield_now();
    }

    // Second call: same closure now returns deadline 1, ticker running -> RuntimeException.
    // If the first call's deadline (1,000,000) carried over, this would succeed despite the ticker.
    let result = binding.dispatch( "root", "burn", &[] );

    stop.store( true, Ordering::Release );
    let _ = handle.join();

    match result {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException on second dispatch, got: {:#?}", other ),
    }
}
