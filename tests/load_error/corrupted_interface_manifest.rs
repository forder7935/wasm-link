use wasm_link::PluginTree;

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [];
}

#[test]
#[should_panic( expected = "Interface root failed to initialise" )]
fn load_error_corrupted_interface_manifest() {

    // The fixtures! macro will panic when parsing the corrupted manifest
    let _ = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins(),
    );

}
