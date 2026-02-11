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

fn dispatch_with_binding_default( binding_deadline: u64, function_deadline: Option<u64>, concurrent_ticker: bool )
    -> Result<Socket<Result<Val, DispatchError>, String>, DispatchError>
{
    let mut config = Config::new();
    config.epoch_interruption( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let function = match function_deadline {
        Some( deadline ) => Function::new( ReturnKind::AssumeNoResources, false ).with_epoch_deadline( deadline ),
        None => Function::new( ReturnKind::AssumeNoResources, false ),
    };

    let binding = Binding::build(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), function )]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    )
        .with_default_epoch_deadline( binding_deadline )
        .build();

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
fn binding_default_is_used_when_function_unspecified() {
    // No function deadline, binding default of 1 with ticker -> exhaustion
    match dispatch_with_binding_default( 1, None, true ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from binding default, got: {:#?}", other ),
    }
}

#[test]
fn function_deadline_overrides_binding_default() {
    // Binding default of 1 but function specifies 1_000_000, no ticker -> completion
    match dispatch_with_binding_default( 1, Some( 1_000_000 ), false ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
