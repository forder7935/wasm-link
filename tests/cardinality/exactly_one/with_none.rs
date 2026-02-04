use wasm_link::{ Engine, Linker, PluginTree, LoadError, Cardinality };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [];
}

#[test]
fn cardinality_test_exactly_one_with_none() {

    let engine = Engine::default();

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

    let linker = Linker::new( &engine );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedCardinalityRequirements( Cardinality::ExactlyOne, 0 ), _ )) => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "{}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    }

}
