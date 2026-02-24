use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    bindings    = [ root: "root" ];
    plugins     = [ burn_fuel: "burn-fuel" ];
}

fn dispatch_with_fuel( fuel: u64 ) -> Result<Socket<Result<Val, wasm_link::DispatchError>, String>, wasm_link::DispatchError> {
    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );
    let plugins = fixtures::plugins( &engine );
    let bindings = fixtures::bindings();

    let plugin_instance = plugins.burn_fuel.plugin
        .with_fuel_limiter( move | _store, _interface, _function, _metadata | fuel )
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let binding = Binding::new(
        bindings.root.package,
        HashMap::from([( bindings.root.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ))]),
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
