
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "resource_limit"] mod resource_limit {

	mod fuel_exhaustion ;
	mod fuel_limiter_closure_args ;
	mod fuel_limiter_per_call_reset ;
	mod fuel_limiter_without_limiter ;
	mod initial_fuel_complex_global ;
	mod initial_fuel_explicit_start ;
	mod initial_fuel_lifetime_budget ;
	mod initial_fuel_memory_initializer ;
	mod initial_fuel_passive_element ;
	mod initial_fuel_table_initializer ;

	mod epoch_exhaustion ;
	mod epoch_limiter_closure_args ;
	mod epoch_limiter_per_call_reset ;
	mod epoch_limiter_without_limiter ;

	mod memory_exhaustion ;
	mod memory_limiter_without_limiter ;

}
