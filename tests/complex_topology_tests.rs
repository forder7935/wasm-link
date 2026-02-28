
include!( "test_utils/fixture_linking.rs" );

#[path = "complex_topology"] mod complex_topology {
	mod deep_nesting ;
	mod shared_dependency ;
	mod multiple_sockets ;
}
