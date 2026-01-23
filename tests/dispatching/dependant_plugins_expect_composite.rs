use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::{ Linker, Val };

#[test]
fn dispatch_test_dependant_plugins_expect_composite() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = initialise_plugin_tree( &test_data_path!( "dispatching", "dependant_plugins_expect_composite" ), &InterfaceId::new( 0 ), engine, &linker ).unwrap();
    warnings.into_iter().for_each(| warning | println!( "{}", warning ));

    match tree.dispatch_function_on_root( "test:dependant-composite/root", "get-composite", true, &[] ) {
        wasm_compose::Socket::ExactlyOne( Ok( Val::Tuple( fields ) )) => {
            assert_eq!( fields[0], Val::U32( 42 ) );
            assert_eq!( fields[1], Val::U32( 24 ) );
        }
        value => panic!( "Expected ExactlyOne( Ok( Tuple( ... ))), found: {:#?}", value ),
    }

}
