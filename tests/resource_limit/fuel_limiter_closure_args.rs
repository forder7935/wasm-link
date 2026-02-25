use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind, Socket, Val };
use wasmtime::Config;

fixtures! {
    bindings    = [ root: "root" ];
    plugins     = [ burn_fuel: "burn-fuel" ];
}

#[test]
fn closure_receives_correct_interface_and_function() {

    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );
    let plugins = fixtures::plugins( &engine );
    let bindings = fixtures::bindings();

    let plugin_instance = plugins.burn_fuel.plugin
        .with_fuel_limiter(| _store, interface, function, _metadata | {
            assert_eq!( interface, "test:fuel/root" );
            assert_eq!( function, "burn" );
            100_000
        })
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let binding = Binding::new(
        bindings.root.package,
        HashMap::from([( bindings.root.name, Interface::new(
            HashMap::from([( "burn".into(), Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
