use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId, Val };

bind_fixtures!( "dispatching", "dependant_plugins_expect_primitive" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn dispatch_test_dependant_plugins_expect_primitive() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![
        InterfaceDir::new( InterfaceId::new( 0 )).unwrap(),
        InterfaceDir::new( InterfaceId::new( 1 )).unwrap(),
    ];
    let plugins = vec![
        PluginDir::new( PluginId::new( "startup".into() )).unwrap(),
        PluginDir::new( PluginId::new( "child".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( InterfaceId::new( 0x_00_00_00_00_u64 ), interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:dependant-primitive/root", "get-primitive", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        value => panic!( "Expected ExactlyOne( Ok( U32( 42 ))), found: {:#?}", value ),
    }

}
