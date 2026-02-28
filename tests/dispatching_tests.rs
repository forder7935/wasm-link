
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "dispatching"] mod dispatching {
	mod dependant_plugins_expect_composite ;
	mod dependant_plugins_expect_primitive ;
	mod single_plugin_expect_composite ;
	mod single_plugin_expect_primitive ;
}