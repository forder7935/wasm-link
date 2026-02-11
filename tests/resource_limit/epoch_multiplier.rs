use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicBool, Ordering }};
use std::thread;
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_multiplier( multiplier: f64, concurrent_ticker: bool )
    -> Result<Socket<Result<Val, DispatchError>, String>, DispatchError>
{
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
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_epoch_deadline( 1_000_000 ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    if concurrent_ticker {
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
    } else {
        binding.dispatch( "root", "burn", &[] )
    }
}

#[test]
fn low_multiplier_causes_exhaustion() {
    // 1_000_000 * 0.0 = 0, immediate exhaustion with ticker
    match dispatch_with_multiplier( 0.0, true ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from multiplier-reduced deadline, got: {:#?}", other ),
    }
}

#[test]
fn normal_multiplier_allows_completion() {
    // 1_000_000 * 1.0 = 1_000_000, no ticker -> completion
    match dispatch_with_multiplier( 1.0, false ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
