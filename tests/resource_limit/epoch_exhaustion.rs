use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_epoch( deadline: u64, concurrent_ticker: bool ) -> Result<Socket<Result<Val, wasm_link::DispatchError>, String>, wasm_link::DispatchError> {

    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_epoch_limiter( move | _store, _interface, _function, _metadata | deadline )
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
        while !started.load( Ordering::Acquire ) {
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
        Ok( Socket::ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from epoch exhaustion, got: {:#?}", other ),
    }
}

#[test]
fn sufficient_epoch_allows_completion() {
    // High deadline without ticker -> completion
    match dispatch_with_epoch( 1_000_000, false ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
