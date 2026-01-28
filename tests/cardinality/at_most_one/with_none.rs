use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId };

bind_fixtures!( "cardinality", "at_most_one", "with_none" );
use fixtures::{ InterfaceDir, PluginDir, FixtureError };

#[test]
fn cardinality_test_at_most_one_with_none() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );
    
    let ( tree, warnings ) = PluginTree::<InterfaceDir, PluginDir>::new::<FixtureError>( vec![], InterfaceId::new( 0x_00_00_00_00_u64 ) );
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
