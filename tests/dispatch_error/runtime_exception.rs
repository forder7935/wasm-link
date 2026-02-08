use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_runtime_exception() {

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let plugin_instance = fixtures::plugin( "test-plugin", &engine ).plugin
		.instantiate( &engine, &linker )
		.expect( "Failed to instantiate plugin" );
	let interface = fixtures::interface( "root" );
	let binding = Binding::new(
		interface.package,
		HashMap::from([( interface.name, interface.interface )]),
		Socket::ExactlyOne( "_".to_string(), plugin_instance ),
	);

	match binding.dispatch( "root", "trap", &[] ) {
		Ok( Socket::ExactlyOne( _, Err( DispatchError::RuntimeException( _ )))) => {}
		value => panic!( "Expected RuntimeException error, found: {:#?}", value ),
	}

}
