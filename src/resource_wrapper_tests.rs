use wasmtime::{ AsContextMut, Engine, Store };
use wasmtime::component::{ Resource, ResourceAny, ResourceTable };

use super::ResourceWrapper ;
use crate::PluginContext ;



struct Context { table: ResourceTable }

impl PluginContext for Context {
	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table }
}

#[test]
fn attached_wrapper_can_be_looked_up_and_dropped() -> Result<(), wasmtime::Error> {
	let mut store = Store::new( &Engine::default(), Context { table: ResourceTable::new() });
	let resource = Resource::<u32>::new_own( 7 );
	let resource = ResourceAny::try_from_resource( resource, &mut store )?;
	let wrapper = ResourceWrapper::new( "plugin".to_string(), resource );
	let handle = wrapper.attach( &mut store.as_context_mut() )?;

	{
		let mut context = store.as_context_mut();
		let found = ResourceWrapper::<String>::from_handle( handle, &mut context )?;
		assert_eq!( found.plugin_id, "plugin" );
		assert_eq!( found.resource_handle, resource );
	}

	let typed = store.data_mut().resource_table().push( std::sync::Arc::new(
		ResourceWrapper::new( "plugin".to_string(), resource )
	))?;
	ResourceWrapper::<String>::drop( store.as_context_mut(), typed.rep() )?;
	assert!( ResourceWrapper::<String>::drop( store.as_context_mut(), typed.rep() ).is_err() );
	Ok(())
}
