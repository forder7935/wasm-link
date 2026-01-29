use wasm_compose::{ Engine, Linker, PluginTree };

bind_fixtures!( "error_handling", "invalid_plugin_omitted" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn error_handling_test_invalid_plugin_omitted() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![
        PluginDir::new( plugins::INVALID ).unwrap(),
        PluginDir::new( plugins::VALID ).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT, interfaces, plugins );
    assert_no_warnings!( warnings );

    if let Err(( err, warnings )) = tree.load( &engine, &linker ) {
        warnings.into_iter().for_each(| warning | println!( "{}", warning ));
        panic!( "{}", err );
    };

}
