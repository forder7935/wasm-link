use wasm_link::{ Engine, Linker, PluginTree };

bind_fixtures!( "cardinality", "at_most_one", "with_none" );
use fixtures::{ InterfaceDir, PluginDir, interfaces };

#[test]
fn cardinality_test_at_most_one_with_none() {

    let engine = Engine::default();
    let linker = Linker::new( &engine );
    
    let interfaces = vec![ InterfaceDir::new( interfaces::ROOT ).unwrap() ];
    let ( tree, warnings ) = PluginTree::<_, PluginDir>::new( interfaces::ROOT.to_string(), interfaces, vec![] );
    assert_no_warnings!( warnings );

    let ( _, warnings ) = tree.load( &engine, &linker ).unwrap();
    assert_no_warnings!( warnings );

}
