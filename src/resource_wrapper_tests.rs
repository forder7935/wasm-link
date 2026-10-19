use wasmtime::component::{Resource, ResourceAny, ResourceTable};
use wasmtime::{AsContextMut, Engine, Store};

use super::ResourceWrapper;
use crate::PluginContext;

struct Context {
	table: ResourceTable,
}

impl PluginContext for Context {
	fn resource_table(&mut self) -> &mut ResourceTable {
		&mut self.table
	}
}

#[test]
fn attached_wrapper_can_be_looked_up_and_dropped() -> Result<(), wasmtime::Error> {
	let mut store = Store::new(
		&Engine::default(),
		Context {
			table: ResourceTable::new(),
		},
	);
	let resource = Resource::<u32>::new_own(7);
	let resource = ResourceAny::try_from_resource(resource, &mut store)?;
	let wrapper = ResourceWrapper::new("plugin".to_string(), resource);
	let handle = wrapper.attach(&mut store.as_context_mut())?;

	{
		let mut context = store.as_context_mut();
		let found = ResourceWrapper::<String>::from_handle(handle, &mut context)?;
		assert_eq!(found.plugin_id, "plugin");
		assert_eq!(found.resource_handle, resource);
	}

	let typed = store
		.data_mut()
		.resource_table()
		.push(std::sync::Arc::new(ResourceWrapper::new(
			"plugin".to_string(),
			resource,
		)))?;
	ResourceWrapper::<String>::drop(store.as_context_mut(), typed.rep())?;
	assert!(ResourceWrapper::<String>::drop(store.as_context_mut(), typed.rep()).is_err());
	Ok(())
}

#[test]
fn failed_handle_conversion_rolls_back_the_resource_table_entry() -> Result<(), wasmtime::Error> {
	let mut store = Store::new(
		&Engine::default(),
		Context {
			table: ResourceTable::new(),
		},
	);
	let resource = ResourceAny::try_from_resource(Resource::<u32>::new_own(7), &mut store)?;
	let mut inserted = None;
	let result =
		ResourceWrapper::new("plugin".to_string(), resource).attach_with(&mut store.as_context_mut(), |resource, _| {
			inserted = Some(resource.rep());
			Err(crate::ResourceCreationError::ResourceHandleConversionFailed.into())
		});
	assert!(matches!(
		result,
		Err(crate::ResourceCreationError::ResourceHandleConversionFailed)
	));
	if let Some(rep) = inserted {
		let handle = Resource::<std::sync::Arc<ResourceWrapper<String>>>::new_borrow(rep);
		assert!(store.data().table.get(&handle).is_err());
	} else {
		return Err(crate::ResourceCreationError::ResourceHandleConversionFailed.into());
	}
	resource.resource_drop(&mut store)?;
	Ok(())
}
