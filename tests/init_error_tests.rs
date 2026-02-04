
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "init_error"] mod init_error {
    mod missing_binding ;
    mod missing_binding_multiple_plugins ;
}
