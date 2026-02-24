use std::collections::{ HashMap, HashSet };
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering } };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

#[test]
fn closure_is_called_per_dispatch_and_fuel_is_reset() {

    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let call_count = Arc::new( AtomicUsize::new( 0 ));
    let call_count_clone = Arc::clone( &call_count );

    let dispatch_call_count = Arc::new( AtomicUsize::new( 0 ));
    let dispatch_call_count_clone = Arc::clone( &dispatch_call_count );

    // First call returns sufficient fuel; subsequent calls return 1 (immediate exhaustion).
    // The closure is not reset between dispatches.
    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_fuel_limiter( move | _store, _interface, _function, _metadata | {
            dispatch_call_count_clone.fetch_add( 1, Ordering::Relaxed );
            if call_count_clone.fetch_add( 1, Ordering::Relaxed ) == 0 { 100_000 } else { 1 }
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

    dispatch_call_count.store( 0, Ordering::Relaxed );
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )) on first dispatch, got: {:#?}", other ),
    }
    assert_eq!( dispatch_call_count.load( Ordering::Relaxed ), 1, "limiter should be called exactly once per dispatch" );

    // If fuel from the first call carried over rather than being reset by the closure,
    // this dispatch would not exhaust fuel immediately
    dispatch_call_count.store( 0, Ordering::Relaxed );
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException on second dispatch, got: {:#?}", other ),
    }
    assert_eq!( dispatch_call_count.load( Ordering::Relaxed ), 1, "limiter should be called exactly once per dispatch" );
}
