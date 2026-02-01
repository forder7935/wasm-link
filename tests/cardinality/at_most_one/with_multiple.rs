use wasm_link::{ Engine, Linker, PluginTree, LoadError, InterfaceCardinality };

fixtures! {
    const ROOT          =   "root" ;
    const INTERFACES    = [ "root" ];
    const PLUGINS       = [ "startup1", "startup2" ];
}

#[test]
fn cardinality_test_at_most_one_with_multiple() {

    let ( tree, warnings ) = PluginTree::new(
        fixtures::ROOT.to_string(),
        fixtures::INTERFACES.clone(),
        fixtures::PLUGINS.clone(),
    );
    assert_no_warnings!( warnings );

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( InterfaceCardinality::AtMostOne, n ), _ )) if n > 1 => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    };

}
