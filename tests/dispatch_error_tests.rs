
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "dispatch_error"] mod dispatch_error {
	mod invalid_interface ;
	mod invalid_function ;
	mod invalid_argument_list ;
	mod runtime_exception ;
}
