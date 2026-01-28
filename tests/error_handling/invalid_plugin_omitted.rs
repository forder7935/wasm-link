use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId };

bind_fixtures!( "error_handling", "invalid_plugin_omitted" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( InterfaceId::new( 0 )).unwrap() ];
    let plugins = vec![
        PluginDir::new( PluginId::new( "invalid".into() )).unwrap(),
        PluginDir::new( PluginId::new( "valid".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( InterfaceId::new( 0x_00_00_00_00_u64 ), interfaces, plugins );
    assert_no_warnings!( warnings );

    if let Err(( err, warnings )) = tree.load( &engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
