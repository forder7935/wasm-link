use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId };

bind_fixtures!( "cardinality", "any", "with_none" );
use fixtures::{ InterfaceDir, PluginDir };

#[test]
fn cardinality_test_any_with_none() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( InterfaceId::new( 0 )).unwrap() ];
    let ( tree, warnings ) = PluginTree::<_, PluginDir>::new( InterfaceId::new( 0 ), interfaces, vec![] );
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
