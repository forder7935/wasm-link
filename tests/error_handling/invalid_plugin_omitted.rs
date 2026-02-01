use wasm_link::{ Engine, Linker, PluginTree };

fixtures! {
    const ROOT          =   "root" ;
    const INTERFACES    = [ "root" ];
    const PLUGINS       = [ "valid", "invalid" ];
}

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let ( tree, warnings ) = PluginTree::new(
        fixtures::ROOT.to_string(),
        fixtures::INTERFACES.clone(),
        fixtures::PLUGINS.clone(),
    );
    assert_no_warnings!( warnings );

    if let Err(( err, warnings )) = tree.load( &engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
