use wasm_link::{ Engine, Linker, PluginTree, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_wrong_argument_count() {

	let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins(),
    );
	assert_no_warnings!( warnings );

	let engine = Engine::default();
	let linker = Linker::new( &engine );

	let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
	assert_no_warnings!( warnings );

	// Call with wrong number of arguments (0 instead of 2)
	// Wasmtime reports this as a RuntimeException
	match tree.dispatch( "test:dispatch-error/root", "add", true, &[] ) {
		Socket::ExactlyOne( Err( DispatchError::RuntimeException( err ) )) if err.to_string().contains( "expected 2 argument" ) => {}
		value => panic!( "Expected RuntimeException about argument count, found: {:#?}", value ),
	}

}
