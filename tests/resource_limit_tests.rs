
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "resource_limit"] mod resource_limit {

    mod fuel_exhaustion ;
    mod fuel_limiter_closure_args ;
    mod fuel_limiter_per_call_reset ;
    mod fuel_limiter_without_limiter ;

    mod epoch_exhaustion ;
    mod epoch_limiter_closure_args ;
    mod epoch_limiter_per_call_reset ;
    mod epoch_limiter_without_limiter ;

    mod memory_exhaustion ;
    mod memory_limiter_without_limiter ;

}
