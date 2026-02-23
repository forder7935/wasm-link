use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

#[test]
fn closure_receives_correct_interface_and_function() {

    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_fuel_limiter(| _store, interface, function, _metadata | {
            assert_eq!( interface, "test:fuel/root" );
            assert_eq!( function, "burn" );
            100_000
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

    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}

#[test]
fn closure_can_vary_fuel_per_function() {

    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    // Return 1 fuel (immediate exhaustion) for any call
    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_fuel_limiter(| _store, _interface, _function, _metadata | 1 )
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

    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from low fuel, got: {:#?}", other ),
    }
}

#[test]
fn no_limiter_means_no_fuel_set() {

    // Without fuel enabled in engine + no limiter -> default wasmtime behavior
    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
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

    // Without consume_fuel enabled, plugin runs fine with no limiter
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
