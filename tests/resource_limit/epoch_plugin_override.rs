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

fn dispatch_with_override( plugin_override: Option<u64>, concurrent_ticker: bool )
    -> Result<Socket<Result<Val, DispatchError>, String>, DispatchError>
{
    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let mut plugin = fixtures::plugin( "burn-fuel", &engine ).plugin;
    if let Some( deadline ) = plugin_override {
        plugin = plugin.with_epoch_deadline_overrides( HashMap::from([
            ( "test:fuel/root".to_string(), HashMap::from([( "burn".to_string(), deadline )]))
        ]));
    }
    let plugin_instance = plugin
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
fn plugin_override_takes_precedence() {
    // Function has 1_000_000 deadline but plugin override is 1 with ticker -> exhaustion
    match dispatch_with_override( Some( 1 ), true ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from plugin override, got: {:#?}", other ),
    }
}

#[test]
fn without_override_function_deadline_is_used() {
    // No plugin override, function has 1_000_000, no ticker -> completion
    match dispatch_with_override( None, false ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
