use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
    const ROOT          =   "root" ;
    const INTERFACES    = [ "root", "dependency" ];
    const PLUGINS       = [ "startup", "child" ];
}

#[test]
fn dispatch_test_dependant_plugins_expect_composite() {

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

    match tree.dispatch( "test:dependant-composite/root", "get-composite", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::Tuple( fields ) )) => {
            assert_eq!( fields[0], Val::U32( 42 ) );
            assert_eq!( fields[1], Val::U32( 24 ) );
        }
        value => panic!( "Expected ExactlyOne( Ok( Tuple( ... ))), found: {:#?}", value ),
    }

}
