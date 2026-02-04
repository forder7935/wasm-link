use wasm_link::{ Engine, PluginTree, PluginTreeError };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "plugin-1", "plugin-2" ];
}

#[test]
fn init_error_missing_binding_multiple_plugins() {

	let engine = Engine::default();

	let ( _tree, errors ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
	);

	assert_eq!( errors.len(), 1 );
	match &errors[ 0 ] {
		PluginTreeError::MissingBinding { binding_id, plugins } => {
			assert_eq!( *binding_id, "missing-binding" );
			assert_eq!( plugins.len(), 2 );
			assert!( plugins.contains( &"plugin-1".to_string() ));
			assert!( plugins.contains( &"plugin-2".to_string() ));
		}
	}

}
