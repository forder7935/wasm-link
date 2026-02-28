
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "resource"] mod resource {
	mod single_plugin ;
	mod dependant_plugins ;
}
