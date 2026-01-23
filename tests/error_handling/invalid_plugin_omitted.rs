use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::Linker ;

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    if let Err(( err, warnings )) = initialise_plugin_tree( &test_data_path!( "error_handling", "invalid_plugin_omitted" ), &InterfaceId::new( 0 ), engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
