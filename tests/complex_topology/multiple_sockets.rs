use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "dep-one", "dep-two", "dep-three" ];
	plugins		= [ "main-plugin", "dep-one-plugin", "dep-two-plugin", "dep-three-plugin" ];
}

#[test]
fn complex_topology_multiple_sockets() {

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

    // Verify the root plugin can be dispatched to (verifies dependencies loaded correctly)
    match tree.dispatch( "root", "get-value", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::U32( 0 ) )) => {}
        value => panic!( "Expected U32(0), found: {:#?}", value ),
    }

}
