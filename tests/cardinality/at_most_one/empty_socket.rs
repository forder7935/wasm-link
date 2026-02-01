use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root", "dependency" ];
	plugins		= [ "consumer" ];
}

#[test]
fn cardinality_test_at_most_one_empty_socket() {

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
