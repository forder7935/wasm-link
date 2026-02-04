use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "interface-b", "interface-c", "interface-d" ];
	plugins		= [ "plugin-a", "plugin-b", "plugin-c", "plugin-d" ];
}

#[test]
fn complex_topology_diamond_dependency() {

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

    // Verify the root plugin can be dispatched to (verifies the diamond loaded correctly)
    match tree.dispatch( "root", "get-value", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::U32( 1 ) )) => {}
        value => panic!( "Expected U32(1), found: {:#?}", value ),
    }

}
