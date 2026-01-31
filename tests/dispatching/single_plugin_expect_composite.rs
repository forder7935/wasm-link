use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

bind_fixtures!( "dispatching", "single_plugin_expect_composite" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn dispatch_test_single_plugin_expect_composite() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![ PluginDir::new( plugins::GET_COMPOSITE ).unwrap() ];
    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT.to_string(), interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:composite/root", "get-composite", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::Tuple( fields ) )) => {
            assert_eq!( fields[0], Val::U32( 42 ) );
            assert_eq!( fields[1], Val::U32( 24 ) );
        }
        value => panic!( "Expected ExactlyOne( Ok( Tuple( ... ))), found: {:#?}", value ),
    }

}
