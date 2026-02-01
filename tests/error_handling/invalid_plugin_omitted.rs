use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "valid", "invalid" ];
}

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins(),
    );
    assert_no_warnings!( warnings );

	let engine = Engine::default();
	let linker = Linker::new( &engine );

    if let Err(( err, warnings )) = tree.load( &engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
