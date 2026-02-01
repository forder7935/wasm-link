use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "level-b", "level-c" ];
	plugins		= [ "plugin-a", "plugin-b", "plugin-c" ];
}

#[test]
fn complex_topology_three_level_chain() {

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

    // Verify the root plugin can be dispatched to (verifies the chain loaded correctly)
    match tree.dispatch( "test:topology/root", "get-value", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::U32( 100 ) )) => {}
        value => panic!( "Expected U32(100), found: {:#?}", value ),
    }

}
