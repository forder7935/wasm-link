use wasm_compose::{ Engine, Linker, PluginTree, Val };

bind_fixtures!( "dispatching", "dependant_plugins_expect_primitive" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn dispatch_test_dependant_plugins_expect_primitive() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![
        InterfaceDir::new( interfaces::ROOT ).unwrap(),
        InterfaceDir::new( interfaces::DEPENDENCY ).unwrap(),
    ];
    let plugins = vec![
        PluginDir::new( plugins::STARTUP ).unwrap(),
        PluginDir::new( plugins::CHILD ).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT, interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    match tree.dispatch( "test:dependant-primitive/root", "get-primitive", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        value => panic!( "Expected ExactlyOne( Ok( U32( 42 ))), found: {:#?}", value ),
    }

}
