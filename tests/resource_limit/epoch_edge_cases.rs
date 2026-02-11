use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_deadline( deadline: u64 ) -> Result<Socket<Result<wasm_link::Val, DispatchError>, String>, DispatchError> {
    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let binding = Binding::new(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_epoch_deadline( deadline ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    // Epoch tests need concurrent ticker
    let stop = Arc::new( AtomicBool::new( false ));
    let stop_clone = Arc::clone( &stop );
    let engine_clone = engine.clone();
    let handle = thread::spawn( move || {
        while !stop_clone.load( Ordering::Relaxed ) {
            engine_clone.increment_epoch();
            thread::yield_now();
        }
    });
    thread::yield_now();

    let result = binding.dispatch( "root", "burn", &[] );

    stop.store( true, Ordering::Relaxed );
    let _ = handle.join();
    result
}

fn dispatch_with_multiplier( base_deadline: u64, multiplier: f64 ) -> Result<Socket<Result<wasm_link::Val, DispatchError>, String>, DispatchError> {
    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_epoch_deadline_multiplier( multiplier )
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let binding = Binding::new(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_epoch_deadline( base_deadline ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    // Epoch tests need concurrent ticker
    let stop = Arc::new( AtomicBool::new( false ));
    let stop_clone = Arc::clone( &stop );
    let engine_clone = engine.clone();
    let handle = thread::spawn( move || {
        while !stop_clone.load( Ordering::Relaxed ) {
            engine_clone.increment_epoch();
            thread::yield_now();
        }
    });
    thread::yield_now();

    let result = binding.dispatch( "root", "burn", &[] );

    stop.store( true, Ordering::Relaxed );
    let _ = handle.join();
    result
}

#[test]
fn zero_deadline_traps_quickly() {
    // Deadline of 0 -> traps after first epoch increment
    match dispatch_with_deadline( 0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from zero deadline, got: {:#?}", other ),
    }
}

#[test]
fn zero_multiplier_traps_quickly() {
    // Any base deadline * 0.0 = 0
    match dispatch_with_multiplier( 1_000_000, 0.0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from zero multiplier, got: {:#?}", other ),
    }
}

#[test]
fn negative_multiplier_traps_quickly() {
    // Negative multiplier treated as 0
    match dispatch_with_multiplier( 1_000_000, -1.0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from negative multiplier, got: {:#?}", other ),
    }
}
