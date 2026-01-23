use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::{ Linker, Val };

#[test]
fn resource_test_wrapper() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = initialise_plugin_tree( &test_data_path!( "resource", "dependant_plugins" ), &InterfaceId::new( 0 ), engine, &linker ).unwrap();
    warnings.into_iter().for_each(| warning | println!( "{}", warning ));

    match tree.dispatch_function_on_root( "test:consumer/root", "get-value", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        wasm_compose::Socket::ExactlyOne( Ok( val )) => panic!( "Expected U32(42), got: {:#?}", val ),
        wasm_compose::Socket::ExactlyOne( Err( err )) => panic!( "Method call failed: {:?}", err ),
        socket => panic!( "Expected ExactlyOne, got: {:#?}", socket ),
    }

}
