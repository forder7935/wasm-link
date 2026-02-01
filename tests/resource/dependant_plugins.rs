use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
    const ROOT          =   "root" ;
    const INTERFACES    = [ "root", "dependency" ];
    const PLUGINS       = [ "consumer", "counter" ];
}

#[test]
fn resource_test_wrapper() {

    let ( tree, warnings ) = PluginTree::new(
        fixtures::ROOT.to_string(),
        fixtures::INTERFACES.clone(),
        fixtures::PLUGINS.clone(),
    );
    assert_no_warnings!( warnings );

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:consumer/root", "get-value", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
