use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val };

fixtures! {
    const ROOT  = "root";
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

#[test]
fn no_limiter_means_no_deadline_set() {

    // Without epoch_interruption enabled + no limiter -> default wasmtime behavior
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

    // Without epoch_interruption enabled, plugin runs fine with no limiter
    match binding.dispatch( "root", "burn", &[] ) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
