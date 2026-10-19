use std::collections::HashMap ;
use wasm_link::{ sync::Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;
use wasmtime::Config ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { startup: "startup" };
}

fn dispatch_with_initial_fuel( fuel: u64 ) -> Result<ExactlyOne<String, Result<Val, wasm_link::DispatchError>>, wasm_link::DispatchError> {
	let mut config = Config::new();
	config.consume_fuel( true );
	let engine = Engine::new( &config ).expect( "failed to create engine" );
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let plugin = plugins.startup.plugin
		.with_initial_fuel( fuel )
		.instantiate( &engine, &linker )
		.expect( "initial fuel should be sufficient for startup" );
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "startup".to_string(), plugin ),
	);
	binding.dispatch( "root", "burn", &[] )
}

#[test]
fn calls_exhaust_the_initial_fuel_remainder_without_a_limiter() {
	match dispatch_with_initial_fuel( 100 ) {
		Ok( ExactlyOne( _, Err( wasm_link::DispatchError::RuntimeException( _ )))) => {}
		other => panic!( "Expected RuntimeException from lifetime fuel exhaustion, got: {:#?}", other ),
	}
}

#[test]
fn sufficient_initial_fuel_covers_startup_and_calls_without_a_limiter() {
	match dispatch_with_initial_fuel( 100_000 ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		other => panic!( "Expected Ok( U32( 42 )), got: {:#?}", other ),
	}
}
