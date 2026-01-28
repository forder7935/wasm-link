use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId };

bind_fixtures!( "error_handling", "invalid_plugin_omitted" );
use fixtures::{ InterfaceDir, PluginDir, FixtureError };

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let plugins = vec![
        PluginDir::new( PluginId::new( "invalid".into() )).unwrap(),
        PluginDir::new( PluginId::new( "valid".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::<InterfaceDir, _>::new::<FixtureError>( plugins, InterfaceId::new( 0x_00_00_00_00_u64 ));
    assert_no_warnings!( warnings );

    if let Err(( err, warnings )) = tree.load( &engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
