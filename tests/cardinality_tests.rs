
include!( "test_utils/fixture_linking.rs" );
include!( "test_utils/assert_no_warnings.rs" );

#[path = "cardinality"] mod cardinality {

    mod at_most_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
    }

    mod exactly_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
    }

    mod at_least_one {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
    }

    mod any {
        mod with_none ;
        mod with_one ;
        mod with_multiple ;
    }
}
