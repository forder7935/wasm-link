use wasm_link::{ Engine, PluginTree };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
#[should_panic( expected = "Plugin test-plugin failed to initialise" )]
fn load_error_corrupted_plugin_manifest() {

    let engine = Engine::default();

    // The fixtures! macro will panic when parsing the corrupted manifest
    let _ = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
    );

}
