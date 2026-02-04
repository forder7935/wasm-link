use wasm_link::{ Engine, Linker, PluginTree, LoadError, Cardinality };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "startup1", "startup2" ];
}

#[test]
fn cardinality_test_at_most_one_with_multiple() {

    let engine = Engine::default();

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

    let linker = Linker::new( &engine );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( Cardinality::AtMostOne, n ), _ )) if n > 1 => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    }

}
