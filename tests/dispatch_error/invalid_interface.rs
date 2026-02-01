use wasm_link::{ Engine, Linker, PluginTree, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_invalid_interface() {

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

	match tree.dispatch( "test:nonexistent/root", "test", true, &[] ) {
		Socket::ExactlyOne( Err( DispatchError::InvalidInterface( _ ) )) => {}
		value => panic!( "Expected InvalidInterface error, found: {:#?}", value ),
	}

}
