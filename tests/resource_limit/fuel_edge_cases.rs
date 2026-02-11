use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, DispatchError };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_fuel( fuel: u64 ) -> Result<Socket<Result<wasm_link::Val, DispatchError>, String>, DispatchError> {
    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let binding = Binding::new(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_fuel( fuel ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    binding.dispatch( "root", "burn", &[] )
}

fn dispatch_with_multiplier( base_fuel: u64, multiplier: f64 ) -> Result<Socket<Result<wasm_link::Val, DispatchError>, String>, DispatchError> {
    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .with_fuel_multiplier( multiplier )
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let binding = Binding::new(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_fuel( base_fuel ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    binding.dispatch( "root", "burn", &[] )
}

#[test]
fn zero_fuel_traps_immediately() {
    match dispatch_with_fuel( 0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from zero fuel, got: {:#?}", other ),
    }
}

#[test]
fn zero_multiplier_traps_immediately() {
    // Any base fuel * 0.0 = 0
    match dispatch_with_multiplier( 100_000, 0.0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from zero multiplier, got: {:#?}", other ),
    }
}

#[test]
fn negative_multiplier_traps_immediately() {
    // Negative multiplier treated as 0
    match dispatch_with_multiplier( 100_000, -1.0 ) {
        Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from negative multiplier, got: {:#?}", other ),
    }
}
