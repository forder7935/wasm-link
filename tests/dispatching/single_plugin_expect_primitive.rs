use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::{ Linker, Val };

#[test]
fn dispatch_test_single_plugin_expect_primitive() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = initialise_plugin_tree( &test_data_path!( "dispatching", "single_plugin_expect_primitive" ), &InterfaceId::new( 0 ), engine, &linker ).unwrap();
    warnings.into_iter().for_each(| warning | println!( "{}", warning ));

    match tree.dispatch_function_on_root( "test:primitive/root", "get-primitive", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::U32( 42 ) )) => {}
        value => panic!( "Expected ExactlyOne( Ok( U32( 42 ))), found: {:#?}", value ),
    }

}
