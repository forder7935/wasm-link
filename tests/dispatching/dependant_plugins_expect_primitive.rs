use std::collections::{ HashMap, HashSet };
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, Remap, ReturnKind, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn dependant_dispatch_encodes_child_errors() -> Result<(), Box<dyn std::error::Error>> {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let child = plugins.child.plugin
		.remap_interfaces( HashMap::from([(
			"root".to_string(),
			Remap::found_as_with_item_resolution_table(
				"root",
				HashMap::from([( "get-value".to_string(), "missing".to_string() )]),
			),
		)]))
		.instantiate( &engine, &linker )?;
	let dependency = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "child".to_string(), child ),
	);
	let startup = plugins.startup.plugin.link( &engine, linker, vec![ dependency ])?;
	let root = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "startup".to_string(), startup ),
	);
	let _ = root.dispatch( "root", "get-primitive", &[] )?;
	Ok(())
}

#[test]
fn method_metadata_rejects_calls_without_resource_argument() -> Result<(), Box<dyn std::error::Error>> {
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
					Function::new( FunctionKind::Method, ReturnKind::AssumeNoResources ),
				)]),
				HashSet::new(),
			),
		)]),
		ExactlyOne( "child".to_string(), child ),
	);
	let startup = plugins.startup.plugin.link( &engine, linker, vec![ dependency ])?;
	let root = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "startup".to_string(), startup ),
	);
	let _ = root.dispatch( "root", "get-primitive", &[] )?;
	Ok(())
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

#[test]
fn dispatch_test_dependant_plugins_expect_primitive() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let child_instance = plugins.child.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate child plugin" );
	let dependency_binding = Binding::new(
		bindings.dependency.package,
		HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
		ExactlyOne( "_".to_string(), child_instance ),
	);

	let startup_instance = plugins.startup.plugin
		.link( &engine, linker.clone(), vec![ dependency_binding ])
		.expect( "Failed to link startup plugin" );
	let root_binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), startup_instance ),
	);

	match root_binding.dispatch( "root", "get-primitive", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
	}

}
