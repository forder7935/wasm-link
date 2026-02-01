use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
    const ROOT          =   "root" ;
    const INTERFACES    = [ "root" ];
    const PLUGINS       = [ "get-value" ];
}

#[test]
fn dispatch_test_single_plugin_expect_primitive() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = PluginTree::new(
        fixtures::ROOT.to_string(),
        fixtures::INTERFACES.clone(),
        fixtures::PLUGINS.clone(),
    );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:primitive/root", "get-primitive", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        value => panic!( "Expected ExactlyOne( Ok( U32( 42 ))), found: {:#?}", value ),
    }

}
