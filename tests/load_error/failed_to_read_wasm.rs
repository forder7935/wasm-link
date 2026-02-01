use wasm_link::{ Engine, Linker, PluginTree, LoadError };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn load_error_failed_to_read_wasm() {

    let ( tree, warnings ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins(),
    );
    assert_no_warnings!( warnings );

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    // The fixture code wraps missing WASM file errors as CorruptedPluginManifest
    // (since it uses a custom PluginData impl that calls Component::from_file)
    match tree.load( &engine, &linker ) {
        Err(( LoadError::FailedToReadWasm( _ ), _ )) => {},
        Err(( LoadError::FailedToLoadComponent( _ ), _ )) => {},
        Err(( LoadError::CorruptedPluginManifest( err ), _ )) if err.to_string().contains( "failed to read" ) => {},
        Err(( err, warnings )) if warnings.is_empty() => panic!( "Unexpected error: {}", err ),
        Err(( err, warnings )) => panic!( "Failed with warnings: {}\n{:?}", err, warnings ),
        Ok( _ ) => panic!( "Expected failure" ),
    };

}
