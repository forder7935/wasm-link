use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_fuel( fuel: u64 ) -> Result<Socket<Result<Val, wasm_link::DispatchError>, String>, wasm_link::DispatchError> {
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

#[test]
fn fuel_exhaustion_returns_runtime_exception() {
    match dispatch_with_fuel( 1 ) {
        Ok( Socket::ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from fuel exhaustion, got: {:#?}", other ),
    }
}

#[test]
fn sufficient_fuel_allows_completion() {
    match dispatch_with_fuel( 100_000 ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
