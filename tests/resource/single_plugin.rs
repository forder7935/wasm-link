use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

bind_fixtures!( "resource", "single_plugin" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn resource_test_method_call() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![ PluginDir::new( plugins::COUNTER ).unwrap() ];

    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT.to_string(), interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    let resource_handle = match tree.dispatch( "test:myresource/root", "[constructor]counter", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::Resource( handle ) )) => handle,
        Socket::ExactlyOne( Ok( val )) => panic!( "Expected resource, got: {:#?}", val ),
        Socket::ExactlyOne( Err( err )) => panic!( "Constructor failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    };

    match tree.dispatch( "test:myresource/root", "[method]counter.get-value", true, &[Val::Resource( resource_handle )] ) {
        Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
