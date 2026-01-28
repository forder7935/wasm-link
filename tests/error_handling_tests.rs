
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "error_handling"] mod error_handling {
    mod invalid_plugin_omitted ;
}