use wasm_compose::{ initialise_plugin_tree, UnrecoverableStartupError, PreloadError, InterfaceCardinality, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::Linker ;

#[test]
fn cardinality_test_exactly_one_with_none() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    match initialise_plugin_tree( &test_data_path!( "cardinality", "exactly_one", "with_none" ), &InterfaceId::new( 0 ), engine, &linker ) {
        Err(( UnrecoverableStartupError::PreloadError(
            PreloadError::FailedCardinalityRequirements( InterfaceCardinality::ExactlyOne, 0 )
        ), _ )) => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed With Warnings: {}\n{:?}", err, warnings ),
        Ok( val ) => panic!( "Expected failure, got: {:#?}", val ),
    };

}
