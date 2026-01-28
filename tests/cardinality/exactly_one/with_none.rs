use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, LoadError, InterfaceCardinality };

bind_fixtures!( "cardinality", "exactly_one", "with_none" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn cardinality_test_exactly_one_with_none() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( InterfaceId::new( 0 )).unwrap() ];
    let ( tree, warnings ) = PluginTree::<_, PluginDir>::new( InterfaceId::new( 0 ), interfaces, vec![] );
    assert_no_warnings!( warnings );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( InterfaceCardinality::ExactlyOne, 0 ), _ )) => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    };

}
