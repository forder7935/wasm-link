use wasm_link::{ Engine, Linker, PluginTree, LoadError };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn load_error_invalid_socket() {

    let engine = Engine::default();

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );
    assert_no_warnings!( warnings );

    let linker = Linker::new( &engine );

    match tree.load( &engine, &linker ) {
        Err(( LoadError::InvalidSocket( id ), _ )) if id == "nonexistent-interface" => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "Unexpected error: {}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    }

}
