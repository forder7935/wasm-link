use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_override( plugin_override: Option<u64> ) -> Result<Socket<Result<Val, wasm_link::DispatchError>, String>, wasm_link::DispatchError> {
    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let mut plugin = fixtures::plugin( "burn-fuel", &engine ).plugin;
    if let Some( fuel ) = plugin_override {
        plugin = plugin.with_fuel_overrides( HashMap::from([
            ( "test:fuel/root".to_string(), HashMap::from([( "burn".to_string(), fuel )]))
        ]));
    }
    let plugin_instance = plugin
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
fn plugin_override_takes_precedence() {
    // Function has 100_000 fuel but plugin override is 1 -> immediate exhaustion
    match dispatch_with_override( Some( 1 )) {
        Ok( Socket::ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from plugin override, got: {:#?}", other ),
    }
}

#[test]
fn without_override_function_fuel_is_used() {
    // No plugin override, function has 100_000 -> completion
    match dispatch_with_override( None ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
