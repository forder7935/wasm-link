use wasm_compose::{ Engine, Linker, PluginTree, Val };

bind_fixtures!( "resource", "single_plugin" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn resource_test_method_call() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![ PluginDir::new( plugins::COUNTER ).unwrap() ];

    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT, interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    let resource_handle = match tree.dispatch( "test:myresource/root", "[constructor]counter", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::Resource( handle ) )) => handle,
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected resource, got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Constructor failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    };

    match tree.dispatch( "test:myresource/root", "[method]counter.get-value", true, &[Val::Resource( resource_handle )] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
