use std::collections::{ HashMap, HashSet };

use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, ReturnKind };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn function_and_resource_names_must_not_collide() -> Result<(), Box<dyn std::error::Error>> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let child = plugins.child.plugin.instantiate( &engine, &linker )?;
	let dependency = Binding::new(
		bindings.dependency.package,
		HashMap::from([(
			bindings.dependency.name,
			Interface::new(
				HashMap::from([(
					"get-value".to_string(),
					Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ),
				)]),
				HashSet::from([ "get-value".to_string() ]),
			),
		)]),
		ExactlyOne( "child".to_string(), child ),
	);
	assert!( plugins.startup.plugin.link( &engine, linker, vec![ dependency ]).is_err() );
	Ok(())
}
