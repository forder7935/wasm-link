use wasm_compose::{ Engine, Linker, PluginTree, InterfaceId, PluginId };

bind_fixtures!( "cardinality", "any", "with_one" );
use fixtures::{ InterfaceDir, PluginDir, FixtureError };

#[test]
fn cardinality_test_any_with_one() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let plugins = vec![ PluginDir::new( PluginId::new( "startup".into() )).unwrap() ];
    let ( tree, warnings ) = PluginTree::<InterfaceDir, _>::new::<FixtureError>( plugins, InterfaceId::new( 0x_00_00_00_00_u64 ));
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
