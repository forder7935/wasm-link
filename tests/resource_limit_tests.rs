
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "resource_limit"] mod resource_limit {
    mod fuel_exhaustion;
    mod fuel_multiplier;
    mod fuel_binding_default;
    mod fuel_plugin_override;
    mod fuel_edge_cases;

    mod epoch_exhaustion;
    mod epoch_multiplier;
    mod epoch_binding_default;
    mod epoch_plugin_override;
    mod epoch_edge_cases;
}
