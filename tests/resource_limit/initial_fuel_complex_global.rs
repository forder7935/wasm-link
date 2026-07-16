use wasm_link::{ Engine, Linker };
use wasmtime::Config ;

fixtures! {
	bindings = {};
	plugins  = { startup: "startup" };
}

fn instantiate_with_fuel( fuel: u64 ) -> Result<(), wasmtime::Error> {
	let mut config = Config::new();
	config.consume_fuel( true );
	let engine = Engine::new( &config ).expect( "failed to create engine" );
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	plugins.startup.plugin
		.with_initial_fuel( fuel )
		.instantiate( &engine, &linker )?;
	Ok(())
}

#[test]
fn complex_global_exhausts_insufficient_initial_fuel() {
	assert!( instantiate_with_fuel( 0 ).is_err() );
}

#[test]
fn complex_global_accepts_sufficient_initial_fuel() {
	instantiate_with_fuel( 100_000 ).expect( "complex global initialization should have sufficient fuel" );
}
