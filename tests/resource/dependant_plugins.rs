use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId, Val };

bind_fixtures!( "resource", "dependant_plugins" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn resource_test_wrapper() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![
        InterfaceDir::new( InterfaceId::new( 0 )).unwrap(),
        InterfaceDir::new( InterfaceId::new( 1 )).unwrap(),
    ];
    let plugins = vec![
        PluginDir::new( PluginId::new( "consumer".into() )).unwrap(),
        PluginDir::new( PluginId::new( "counter".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( InterfaceId::new( 0x_00_00_00_00_u64 ), interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:consumer/root", "get-value", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
