
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "resource_limit"] mod resource_limit {

    mod fuel_exhaustion ;
    mod fuel_limiter ;

    mod epoch_exhaustion ;
    mod epoch_limiter ;

}
