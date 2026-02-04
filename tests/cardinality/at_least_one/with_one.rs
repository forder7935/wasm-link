use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "startup" ];
}

#[test]
fn cardinality_test_at_least_one_with_one() {

	let engine = Engine::default();

	let	( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
        fixtures::interfaces(),
        fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

	let linker = Linker::new( &engine );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
