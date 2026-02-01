use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "startup1", "startup2" ];
}

#[test]
fn cardinality_test_at_least_one_with_multiple() {

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins(),
    );
    assert_no_warnings!( warnings );

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
