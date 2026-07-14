use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, PluginInstance, Remap, Val, nem };
use wasm_link::cardinality::{ Any, AtLeastOne, AtMostOne };

use crate::fixture_linking::TestContext ;

fixtures! {
	bindings = { root: "root" };
	plugins  = {
		canonical: "canonical",
		remapped: "remapped",
	};
}

#[test]
fn dispatch_allows_different_plugins_to_use_different_export_names() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let canonical = plugins.canonical.plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate canonical plugin" );

	let remapped = plugins.remapped.plugin
		.remap_interfaces( HashMap::from([(
			"root".to_string(),
			Remap::found_as_with_item_resolution_table(
				"remapped",
				HashMap::from([( "get-value".to_string(), "legacy-get-value".to_string() )]),
			),
		)]))
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate remapped plugin" );

	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Any( HashMap::from([
			( "canonical".to_string(), canonical ),
			( "remapped".to_string(), remapped ),
		])),
	);

	let result = binding.dispatch( "root", "get-value", &[] ).expect( "dispatch failed" );

	assert_eq!( result.0.len(), 2 );

	match result.0.get( "canonical" ) {
		Some( Ok( Val::U32( 40 ))) => {}
		value => panic!( "Expected canonical plugin to return U32( 40 ), found: {value:#?}" ),
	}

	match result.0.get( "remapped" ) {
		Some( Ok( Val::U32( 43 ))) => {}
		value => panic!( "Expected remapped plugin to return U32( 43 ), found: {value:#?}" ),
	}

}

#[test]
fn links_each_type_erased_binding_cardinality() -> Result<(), wasmtime::Error> {
	let engine = Engine::default();

	let bindings = fixtures::bindings();
	let instance = fixtures::plugins( &engine ).canonical.plugin
		.instantiate( &engine, &Linker::new( &engine ))?;
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		wasm_link::cardinality::ExactlyOne( "canonical".to_string(), instance ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).canonical.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let binding: Binding<String, TestContext, AtMostOne<String, PluginInstance<TestContext>>> = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		AtMostOne( None ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).canonical.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let instance = fixtures::plugins( &engine ).canonical.plugin
		.instantiate( &engine, &Linker::new( &engine ))?;
	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		AtLeastOne( nem! { "canonical".to_string() => instance }),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).canonical.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;

	let bindings = fixtures::bindings();
	let binding: Binding<String, TestContext, Any<String, PluginInstance<TestContext>>> = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		Any( HashMap::new() ),
	);
	let socket = binding.into_any();
	let plugin = fixtures::plugins( &engine ).canonical.plugin;
	let _ = plugin.link( &engine, Linker::new( &engine ), vec![ socket.clone() ])?;
	Ok(())
}
