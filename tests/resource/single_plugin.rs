use wasm_link::{ Engine, Linker, PluginTree, Val, Socket };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "counter" ];
}

#[test]
fn resource_test_method_call() {

    let engine = Engine::default();

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

    let linker = Linker::new( &engine );

    let ( tree, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

    let resource_handle = match tree.dispatch( "root", "[constructor]counter", true, &[] ) {
        Socket::ExactlyOne( Ok( Val::Resource( handle ) )) => handle,
        Socket::ExactlyOne( Ok( val )) => panic!( "Expected resource, got: {:#?}", val ),
        Socket::ExactlyOne( Err( err )) => panic!( "Constructor failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    };

    match tree.dispatch( "root", "[method]counter.get-value", true, &[Val::Resource( resource_handle )] ) {
        Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
