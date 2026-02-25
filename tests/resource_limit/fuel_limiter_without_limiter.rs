use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind, Socket, Val };

fixtures! {
    bindings    = [ root: "root" ];
    plugins     = [ burn_fuel: "burn-fuel" ];
}

#[test]
fn no_limiter_means_no_fuel_set() {

    // Without fuel enabled in engine + no limiter -> default wasmtime behavior
    let engine = Engine::default();
    let linker = Linker::new( &engine );
    let plugins = fixtures::plugins( &engine );
    let bindings = fixtures::bindings();

    let plugin_instance = plugins.burn_fuel.plugin
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

    // Without consume_fuel enabled, plugin runs fine with no limiter
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
