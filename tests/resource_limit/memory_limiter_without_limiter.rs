use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind, Socket, Val };

fixtures! {
    bindings    = [ root: "root" ];
    plugins     = [ grow_memory: "grow-memory" ];
}

#[test]
fn no_limiter_means_memory_grows_freely() {
    let engine = Engine::default();
    let linker = Linker::new( &engine );
    let plugins = fixtures::plugins( &engine );
    let bindings = fixtures::bindings();

    let plugin_instance = plugins.grow_memory.plugin
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let binding = Binding::new(
        bindings.root.package,
        HashMap::from([( bindings.root.name, Interface::new(
            HashMap::from([( "grow-memory".into(), Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ))]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    );

    match binding.dispatch( "root", "grow-memory", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::S32( 1 )))) => {}
        other => panic!( "Expected Ok( S32( 1 )) from unconstrained memory growth, got: {:#?}", other ),
    }
}
