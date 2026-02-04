use wasm_link::{ Engine, Linker, PluginTree, DispatchError, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn dispatch_error_runtime_exception() {

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

	match tree.dispatch( "root", "trap", true, &[] ) {
		Socket::ExactlyOne( Err( DispatchError::RuntimeException( _ ) )) => {}
		value => panic!( "Expected RuntimeException error, found: {:#?}", value ),
	}

}
