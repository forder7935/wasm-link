#[macro_export]
macro_rules! assert_no_warnings {
	( $warnings:expr ) => {
		if !$warnings.is_empty() { panic!( "Produced warnings: {:?}", $warnings ) }
	};
}
