use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [];
}

#[test]
fn cardinality_test_at_most_one_with_none() {

    let engine = Engine::default();

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

    let linker = Linker::new( &engine );
    
    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
