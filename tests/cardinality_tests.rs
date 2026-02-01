
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "cardinality"] mod cardinality {

    mod at_most_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
        mod empty_socket ;
    }

    mod exactly_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
        mod empty_socket ;
    }

    mod at_least_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
        mod empty_socket ;
    }

    mod any {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
        mod empty_socket ;
    }
}
