use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::{ Linker, Val };

#[test]
fn resource_test_method_call() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = initialise_plugin_tree( &test_data_path!( "resource", "single_plugin" ), &InterfaceId::new( 0 ), engine, &linker ).unwrap();
    warnings.into_iter().for_each(| warning | println!( "{}", warning ));

    let resource_handle = match tree.dispatch_function_on_root( "test:myresource/root", "[constructor]counter", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::Resource( handle ) )) => handle,
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected resource, got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Constructor failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    };

    match tree.dispatch_function_on_root( "test:myresource/root", "[method]counter.get-value", true, &[Val::Resource( resource_handle )] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
