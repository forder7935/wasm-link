use wasm_compose::{ Engine, Linker, PluginTree, LoadError, InterfaceCardinality };

bind_fixtures!( "cardinality", "exactly_one", "with_multiple" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn cardinality_test_exactly_one_with_multiple() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![
        PluginDir::new( plugins::STARTUP ).unwrap(),
        PluginDir::new( plugins::STARTUP2 ).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT, interfaces, plugins );
    assert_no_warnings!( warnings );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( InterfaceCardinality::ExactlyOne, n ), _ )) if n > 1 => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected PluginLoadError( FailedCardinalityRequirements( ExactlyOne, n )) where n > 1" ),
    };

}
