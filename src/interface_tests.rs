use crate::{ FunctionKind, ReturnKind };



#[test]
fn return_kinds_describe_their_resource_handling_contract() {
	assert_eq!( ReturnKind::Void.to_string(), "Function returns no data" );
	assert_eq!( ReturnKind::MayContainResources.to_string(), "Return type may contain resources" );
	assert_eq!( ReturnKind::AssumeNoResources.to_string(), "Function is assumed to not return any resources" );
}

#[test]
fn runtime_function_metadata_has_matching_accessors() {
	let sync = crate::sync::Function::new(
		FunctionKind::Method,
		ReturnKind::MayContainResources,
	);
	assert_eq!( sync.kind(), FunctionKind::Method );
	assert_eq!( sync.return_kind(), ReturnKind::MayContainResources );
	assert!( !sync.is_async() );

	let concurrent = crate::concurrent::Function::new(
		FunctionKind::Freestanding,
		ReturnKind::Void,
	);
	assert_eq!( concurrent.kind(), FunctionKind::Freestanding );
	assert_eq!( concurrent.return_kind(), ReturnKind::Void );
	assert!( !concurrent.is_async() );
}
