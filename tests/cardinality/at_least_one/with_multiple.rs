use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId };

bind_fixtures!( "cardinality", "at_least_one", "with_multiple" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn cardinality_test_at_least_one_with_multiple() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( InterfaceId::new( 0 )).unwrap() ];
    let plugins = vec![
        PluginDir::new( PluginId::new( "startup".into() )).unwrap(),
        PluginDir::new( PluginId::new( "startup2".into() )).unwrap(),
    ];
    let ( tree, warnings ) = PluginTree::new( InterfaceId::new( 0x_00_00_00_00_u64 ), interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
