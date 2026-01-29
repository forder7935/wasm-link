use wasm_compose::{ Engine, Linker, PluginTree };

bind_fixtures!( "cardinality", "any", "with_one" );
use fixtures::{ InterfaceDir, PluginDir, interfaces, plugins };

#[test]
fn cardinality_test_any_with_one() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );

    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let plugins = vec![ PluginDir::new( plugins::STARTUP ).unwrap() ];
    let ( tree, warnings ) = PluginTree::new( interfaces::ROOT, interfaces, plugins );
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
