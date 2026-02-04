use wasm_link::{ Engine, PluginTree, PluginTreeError };

fixtures! {
	const ROOT	=   "root" ;
	interfaces	= [ "root" ];
	plugins		= [ "test-plugin" ];
}

#[test]
fn init_error_missing_binding() {

	let engine = Engine::default();

	let ( _tree, errors ) = PluginTree::new(
		fixtures::ROOT.to_string(),
		fixtures::interfaces(),
		fixtures::plugins( &engine ),
	);

	assert_eq!( errors.len(), 1 );
	match &errors[ 0 ] {
		PluginTreeError::MissingBinding { binding_id, plugins } => {
			assert_eq!( *binding_id, "nonexistent-binding" );
			assert_eq!( plugins, &vec![ "test-plugin".to_string() ]);
		}
	}

}
