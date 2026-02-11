use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_multiplier( multiplier: f64 ) -> Result<Socket<Result<Val, wasm_link::DispatchError>, String>, wasm_link::DispatchError> {
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
            HashMap::from([( "burn".into(), Function::new( ReturnKind::AssumeNoResources, false ).with_fuel( 100_000 ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    binding.dispatch( "root", "burn", &[] )
}

#[test]
fn low_multiplier_causes_exhaustion() {
    // 100_000 * 0.0 = 0, immediate exhaustion
    match dispatch_with_multiplier( 0.0 ) {
        Ok( Socket::ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from multiplier-reduced fuel, got: {:#?}", other ),
    }
}

#[test]
fn normal_multiplier_allows_completion() {
    // 100_000 * 1.0 = 100_000, plenty of fuel
    match dispatch_with_multiplier( 1.0 ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
