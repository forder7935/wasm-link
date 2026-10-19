use std::collections::HashMap ;
use wasm_link::{ sync::Binding, Engine, Linker, Remap, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root" };
	plugins  = { root: "root" };
}

#[test]
fn dispatch_remaps_interface_name() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();

	let plugin = plugins.root.plugin
		.remap_interfaces( HashMap::from([
			( "root".to_string(), Remap::found_as( "remapped" )),
		]))
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );

	let binding = Binding::new(
		bindings.root.package,
		HashMap::from([( bindings.root.name, bindings.root.spec )]),
		ExactlyOne( "_".to_string(), plugin ),
	);

	match binding.dispatch( "root", "get-value", &[] ) {
		Ok( ExactlyOne( _, Ok( Val::U32( 41 )))) => {}
		value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 41 )))), found: {:#?}", value ),
	}

}
