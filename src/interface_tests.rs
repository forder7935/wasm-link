use crate::ReturnKind ;



#[test]
fn return_kinds_describe_their_resource_handling_contract() {
	assert_eq!( ReturnKind::Void.to_string(), "Function returns no data" );
	assert_eq!( ReturnKind::MayContainResources.to_string(), "Return type may contain resources" );
	assert_eq!( ReturnKind::AssumeNoResources.to_string(), "Function is assumed to not return any resources" );
}
