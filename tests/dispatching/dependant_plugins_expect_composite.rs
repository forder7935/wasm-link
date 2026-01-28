use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId, Val };

bind_fixtures!( "dispatching", "dependant_plugins_expect_composite" );
use fixtures::{ InterfaceDir, PluginDir, FixtureError };

#[test]
fn dispatch_test_dependant_plugins_expect_composite() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let plugins = vec![
        PluginDir::new( PluginId::new( "startup".into() )).unwrap(),
        PluginDir::new( PluginId::new( "child".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::<InterfaceDir, _>::new::<FixtureError>( plugins, InterfaceId::new( 0x_00_00_00_00_u64 ));
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:dependant-composite/root", "get-composite", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::Tuple( fields ) )) => {
            assert_eq!( fields[0], Val::U32( 42 ) );
            assert_eq!( fields[1], Val::U32( 24 ) );
        }
        value => panic!( "Expected ExactlyOne( Ok( Tuple( ... ))), found: {:#?}", value ),
    }

}
