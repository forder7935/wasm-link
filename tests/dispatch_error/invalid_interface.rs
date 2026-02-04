use wasm_link::{ Engine, Linker, PluginTree, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_invalid_interface() {

	let engine = Engine::default();

	let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
	);
	assert_no_warnings!( warnings );

	let linker = Linker::new( &engine );

	let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
	assert_no_warnings!( warnings );

	match tree.dispatch( "nonexistent", "test", true, &[] ) {
		Socket::ExactlyOne( Err( DispatchError::InvalidInterfacePath( _ ) )) => {}
		value => panic!( "Expected InvalidInterface error, found: {:#?}", value ),
	}

}
