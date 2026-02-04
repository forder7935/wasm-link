
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "load_error"] mod load_error {
    mod invalid_socket ;
    mod loop_detected ;
    mod corrupted_interface_manifest ;
    mod corrupted_plugin_manifest ;
}
