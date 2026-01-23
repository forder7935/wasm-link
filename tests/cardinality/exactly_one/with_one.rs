use pipe_trait::Pipe ;

use wasm_compose::{ initialise_plugin_tree, InterfaceId };
use wasm_compose::utils::deconstruct_partial_result ;
use wasmtime::Engine ;
use wasmtime::component::Linker ;

#[test]
fn cardinality_test_exactly_one_with_one() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );
    let ( result, warnings ) = initialise_plugin_tree( &test_data_path!( "cardinality", "exactly_one", "with_one" ), &InterfaceId::new( 0 ), engine, &linker )
        .pipe( deconstruct_partial_result );

    match result {
        Ok(_) if warnings.is_empty() => {},
        Ok(_) => panic!( "Produced Warnings: {:?}", warnings ),
        Err( err ) if warnings.is_empty() => panic!( "{}", err ),
        Err( err ) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
    }

}
