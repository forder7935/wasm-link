use std::collections::HashMap ;

use wasm_link::{ Binding, Engine, Linker };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn duplicate_socket_interfaces_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let child = plugins.child.plugin.instantiate( &engine, &linker )?;
	let dependency = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "child".to_string(), child ),
	);
	assert!( plugins.startup.plugin.link(
		&engine,
		linker,
		vec![ dependency.clone(), dependency ],
	).is_err() );
	Ok(())
}
