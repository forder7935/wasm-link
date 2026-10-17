use wasmtime::component::{Component, FutureReader, Linker, ResourceTable, StreamReader, Val};
use wasmtime::{AsContextMut, Config, Engine, Store};

use crate::errors::RouteError;
fn wrap_resources(value: Val, id: String, store: &mut wasmtime::StoreContextMut<Context>) -> Result<Val, RouteError> {
	let cleanup = std::sync::Arc::new(crate::plugin_instance::CleanupQueue::default());
	super::wrap_owned_resources(
		crate::plugin_instance::OwnedValue {
			value: Some(value),
			cleanup: cleanup.clone(),
		},
		id,
		store,
		&cleanup,
	)
}
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
fn wraps_nested_component_values_without_resources() -> Result<(), RouteError> {
	let mut store = Store::new(
		&Engine::default(),
		Context {
			table: ResourceTable::new(),
		},
	);
	let values = [
		Val::Bool(true),
		Val::List(vec![Val::U32(1)]),
		Val::FixedLengthList(vec![Val::U32(1)]),
		Val::Map(vec![(Val::String("key".to_string()), Val::U32(2))]),
		Val::Record(vec![("field".to_string(), Val::U32(3))]),
		Val::Tuple(vec![Val::U32(4)]),
		Val::Variant("value".to_string(), Some(Box::new(Val::U32(5)))),
		Val::Option(Some(Box::new(Val::U32(6)))),
		Val::Result(Ok(Some(Box::new(Val::U32(7))))),
		Val::Result(Err(Some(Box::new(Val::U32(8))))),
	];

	values
		.into_iter()
		.try_for_each(|value| wrap_resources(value, "plugin".to_string(), &mut store.as_context_mut()).map(drop))?;
	Ok(())
}

#[test]
fn rejects_async_values_during_cross_plugin_transfer() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = Config::new();
	config.concurrency_support(true);
	let engine = Engine::new(&config)?;
	let mut store = Store::new(
		&engine,
		Context {
			table: ResourceTable::new(),
		},
	);
	let future =
		FutureReader::new(&mut store, async { Ok::<_, wasmtime::Error>(1_u32) })?.try_into_future_any(&mut store)?;
	let stream = StreamReader::new(&mut store, vec![1_u32])?.try_into_stream_any(&mut store)?;

	assert!(matches!(
		wrap_resources( Val::Future( future ), "plugin".to_string(), &mut store.as_context_mut() ),
		Err( RouteError::Call(crate::CallError::UnsupportedType( name ))) if name == "future"
	));
	assert!(matches!(
		wrap_resources( Val::Stream( stream ), "plugin".to_string(), &mut store.as_context_mut() ),
		Err( RouteError::Call(crate::CallError::UnsupportedType( name ))) if name == "stream"
	));
	Ok(())
}

#[test]
fn rejects_error_contexts_during_cross_plugin_transfer() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on(async {
		let mut config = Config::new();
		config.wasm_component_model_async(true);
		config.wasm_component_model_error_context(true);
		let engine = Engine::new(&config)?;
		let component = Component::from_file(
			&engine,
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/linker/error_context.wat"),
		)?;
		let linker = Linker::<Context>::new(&engine);
		let mut store = Store::new(
			&engine,
			Context {
				table: ResourceTable::new(),
			},
		);
		let instance = linker.instantiate_async(&mut store, &component).await?;
		let function = instance
			.get_func(&mut store, "make-error-context")
			.ok_or("missing make-error-context export")?;
		let mut results = [Val::Bool(false)];
		function.call_async(&mut store, &[], &mut results).await?;
		assert!(matches!(
			wrap_resources( results[0].clone(), "plugin".to_string(), &mut store.as_context_mut() ),
			Err( RouteError::Call(crate::CallError::UnsupportedType( name ))) if name == "error-context"
		));
		Ok::<_, Box<dyn std::error::Error>>(())
	})
}

#[test]
fn wrapping_failure_releases_partial_wrappers_without_double_dropping_origin() -> Result<(), Box<dyn std::error::Error>>
{
	use std::sync::Arc;
	use wasmtime::component::{Resource, ResourceAny};
	let mut store = Store::new(
		&Engine::default(),
		Context {
			table: ResourceTable::new(),
		},
	);
	let resource = ResourceAny::try_from_resource(Resource::<u32>::new_own(7), &mut store)?;
	let future =
		FutureReader::new(&mut store, async { Ok::<_, wasmtime::Error>(1_u32) })?.try_into_future_any(&mut store)?;
	for unsupported in [
		Val::Option(Some(Box::new(Val::Future(future.clone())))),
		Val::Result(Ok(Some(Box::new(Val::Future(future.clone()))))),
		Val::Result(Err(Some(Box::new(Val::Future(future.clone()))))),
	] {
		assert!(wrap_resources(unsupported, "plugin".into(), &mut store.as_context_mut()).is_err());
	}
	let origin = Arc::new(crate::plugin_instance::CleanupQueue::default());
	let caller = crate::plugin_instance::CleanupQueue::default();
	let value = crate::plugin_instance::OwnedValue {
		value: Some(Val::Tuple(vec![Val::Resource(resource), Val::Future(future.clone())])),
		cleanup: origin.clone(),
	};
	assert!(super::wrap_owned_resources(value, "plugin".to_string(), &mut store.as_context_mut(), &caller).is_err());
	// Failed wrapping keeps the origin guard armed and leaves wrappers unarmed.
	let origin_resources = origin.take();
	assert_eq!(origin_resources, [resource]);
	let wrappers = caller.take();
	assert_eq!(wrappers.len(), 1);
	for wrapper in wrappers {
		wrapper.resource_drop(&mut store)?;
	}
	assert_eq!(origin.take(), [] as [ResourceAny; 0]);
	resource.resource_drop(&mut store)?;
	Ok(())
}
