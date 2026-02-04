use wasm_link::{ Engine, Linker, PluginTree, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_invalid_function() {

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

	match tree.dispatch( "root", "nonexistent-function", true, &[] ) {
		Socket::ExactlyOne( Err( DispatchError::InvalidFunction( _ ) )) => {}
		value => panic!( "Expected InvalidFunction error, found: {:#?}", value ),
	}

}
