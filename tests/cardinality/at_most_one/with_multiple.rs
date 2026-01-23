use wasm_compose::{ initialise_plugin_tree, UnrecoverableStartupError, PreloadError, InterfaceCardinality, InterfaceId };
use wasmtime::Engine ;
use wasmtime::component::Linker ;

#[test]
fn cardinality_test_at_most_one_with_multiple() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    match initialise_plugin_tree( &test_data_path!( "cardinality", "at_most_one", "with_multiple" ), &InterfaceId::new( 0 ), engine, &linker ) {
        Err(( UnrecoverableStartupError::PreloadError(
            PreloadError::FailedCardinalityRequirements( InterfaceCardinality::AtMostOne, n )
        ), _ )) if n > 1 => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed With Warnings: {}\n{:?}", err, warnings ),
        Ok( val ) => panic!( "Expected failure, got: {:#?}", val ),
    };

}
