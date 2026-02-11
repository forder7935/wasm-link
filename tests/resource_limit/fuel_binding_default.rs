use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, Interface, Linker, ReturnKind, Socket, Val, DispatchError };
use wasmtime::Config ;

fixtures! {
    const ROOT  =   "root" ;
    interfaces  = [ "root" ];
    plugins     = [ "burn-fuel" ];
}

fn dispatch_with_binding_default( binding_fuel: u64, function_fuel: Option<u64> )
    -> Result<Socket<Result<Val, DispatchError>, String>, DispatchError>
{
    let mut config = Config::new();
    config.consume_fuel( true );
    let engine = Engine::new( &config ).expect( "failed to create engine" );
    let linker = Linker::new( &engine );

    let plugin_instance = fixtures::plugin( "burn-fuel", &engine ).plugin
        .instantiate( &engine, &linker )
        .expect( "failed to instantiate plugin" );

    let interface = fixtures::interface( "root" );
    let function = match function_fuel {
        Some( fuel ) => Function::new( ReturnKind::AssumeNoResources, false ).with_fuel( fuel ),
        None => Function::new( ReturnKind::AssumeNoResources, false ),
    };

    let binding = Binding::build(
        interface.package,
        HashMap::from([( interface.name, Interface::new(
            HashMap::from([( "burn".into(), function )]),
            HashSet::new(),
        ))]),
        Socket::ExactlyOne( "_".to_string(), plugin_instance ),
    )
        .with_default_fuel( binding_fuel )
        .build();

    binding.dispatch( "root", "burn", &[] )
}

#[test]
fn binding_default_is_used_when_function_unspecified() {
    // No function fuel, binding default of 1 -> immediate exhaustion
    match dispatch_with_binding_default( 1, None ) {
        Ok( Socket::ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
        other => panic!( "Expected RuntimeException from binding default, got: {:#?}", other ),
    }
}

#[test]
fn function_fuel_overrides_binding_default() {
    // Binding default of 1 but function specifies 100_000 -> completion
    match dispatch_with_binding_default( 1, Some( 100_000 )) {
        Ok( Socket::ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
        other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
    }
}
