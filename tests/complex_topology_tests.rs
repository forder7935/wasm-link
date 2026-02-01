
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "complex_topology"] mod complex_topology {
    mod three_level_chain ;
    mod diamond_dependency ;
    mod multiple_sockets ;
}
