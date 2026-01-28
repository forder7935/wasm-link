use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId, LoadError, InterfaceCardinality };

bind_fixtures!( "cardinality", "at_most_one", "with_multiple" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn cardinality_test_at_most_one_with_multiple() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( InterfaceId::new( 0 )).unwrap() ];
    let plugins = vec![
        PluginDir::new( PluginId::new( "startup".into() )).unwrap(),
        PluginDir::new( PluginId::new( "startup2".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( InterfaceId::new( 0x_00_00_00_00_u64 ), interfaces, plugins );
    assert_no_warnings!( warnings );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( InterfaceCardinality::AtMostOne, n ), _ )) if n > 1 => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    };

}
