use super::ensure_supported_value;
use crate::{CallError, PluginContext};
use wasmtime::component::{Component, FutureReader, Linker, ResourceTable, StreamReader, Val};
use wasmtime::{Config, Engine, Store};
struct Context {
	table: ResourceTable,
}
impl PluginContext for Context {
	fn resource_table(&mut self) -> &mut ResourceTable {
		&mut self.table
	}
}
#[test]
fn accepts_nested_component_values() -> Result<(), CallError> {
	let value = Val::Record(vec![
		("list".to_string(), Val::List(vec![Val::U32(1)])),
		("fixed-length-list".to_string(), Val::FixedLengthList(vec![Val::U32(1)])),
		("tuple".to_string(), Val::Tuple(vec![Val::U32(2)])),
		(
			"map".to_string(),
			Val::Map(vec![(Val::String("key".to_string()), Val::U32(3))]),
		),
		(
			"variant".to_string(),
			Val::Variant("some".to_string(), Some(Box::new(Val::U32(4)))),
		),
		("option".to_string(), Val::Option(Some(Box::new(Val::U32(5))))),
		("ok".to_string(), Val::Result(Ok(Some(Box::new(Val::U32(6)))))),
		("err".to_string(), Val::Result(Err(Some(Box::new(Val::U32(7)))))),
	]);
	ensure_supported_value(&value)
}

#[test]
fn rejects_future_and_stream_values() -> Result<(), Box<dyn std::error::Error>> {
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
		ensure_supported_value( &Val::Future( future )),
		Err( CallError::UnsupportedType( name )) if name == "future"
	));
	assert!(matches!(
		ensure_supported_value( &Val::Stream( stream )),
		Err( CallError::UnsupportedType( name )) if name == "stream"
	));
	Ok(())
}

#[test]
fn rejects_error_context_values() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on(async {
		let mut config = Config::new();
		config.wasm_component_model_async(true);
		config.wasm_component_model_error_context(true);
		let engine = Engine::new(&config)?;
		let component = Component::from_file(
			&engine,
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/plugin_instance/error_context.wat"),
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
			ensure_supported_value( &results[0] ),
			Err( CallError::UnsupportedType( name )) if name == "error-context"
		));
		Ok(())
	})
}

#[test]
fn successful_send_followed_by_receiver_drop_retains_resource_cleanup() -> Result<(), Box<dyn std::error::Error>> {
	let engine = Engine::default();
	let mut store = Store::new(&engine, ());
	let resource = wasmtime::component::ResourceAny::try_from_resource(
		wasmtime::component::Resource::<u32>::new_own(7),
		&mut store,
	)?;
	let cleanup = std::sync::Arc::new(super::CleanupQueue::default());
	let value = super::OwnedValue {
		value: Some(Val::Tuple(vec![Val::Option(Some(Box::new(Val::Resource(resource))))])),
		cleanup: cleanup.clone(),
	};
	let (sender, receiver) = futures::channel::oneshot::channel();
	assert!(sender.send(value).is_ok());
	assert!(cleanup
		.resources
		.lock()
		.map_err(|_| "cleanup lock poisoned")?
		.is_empty());
	drop(receiver);
	let resources = std::mem::take(&mut *cleanup.resources.lock().map_err(|_| "cleanup lock poisoned")?);
	assert_eq!(resources.len(), 1);
	for resource in resources {
		resource.resource_drop(&mut store)?;
	}
	Ok(())
}

#[test]
fn nested_result_cleanup_visits_resources_once() -> Result<(), wasmtime::Error> {
	let mut store = Store::new(&Engine::default(), ());
	let resource = wasmtime::component::ResourceAny::try_from_resource(
		wasmtime::component::Resource::<u32>::new_own(7),
		&mut store,
	)?;
	let value = Val::Map(vec![(
		Val::String("key".into()),
		Val::Record(vec![(
			"value".into(),
			Val::Result(Ok(Some(Box::new(Val::Resource(resource))))),
		)]),
	)]);
	let mut visited = Vec::new();
	super::visit_resources(value, &mut |r| visited.push(r));
	assert_eq!(visited, [resource]);
	resource.resource_drop(&mut store)?;
	Ok(())
}

#[test]
#[allow(clippy::waker_clone_wake)] // Exercise both Wake entry points deliberately.
fn blocking_adapter_handles_owned_and_borrowed_wakes() {
	for owned in [false, true] {
		let mut first = true;
		super::block_on(futures::future::poll_fn(|cx| {
			if first {
				first = false;
				if owned {
					cx.waker().clone().wake();
				} else {
					cx.waker().wake_by_ref();
				}
				std::task::Poll::Pending
			} else {
				std::task::Poll::Ready(())
			}
		}));
	}
}

#[test]
fn closing_the_last_request_sender_stops_an_idle_driver() -> Result<(), Box<dyn std::error::Error>> {
	use super::InstanceHandle;
	super::block_on(async {
		let engine = Engine::default();
		let component = Component::from_file(
			&engine,
			concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/tests/dispatching/dependant_plugins_async/plugins/child-sync/root.wat"
			),
		)?;
		let instance = crate::Plugin::new(
			component,
			Context {
				table: ResourceTable::new(),
			},
		)
		.instantiate_async(&engine, &Linker::new(&engine))
		.await?;
		let handle = instance.handle();
		let driver = handle.inner.driver.clone();
		drop(handle);
		drop(instance);
		assert_eq!(driver.await, "store stopped");
		Ok(())
	})
}

#[test]
fn cleanup_reservation_wakes_an_async_admission_waiter() -> Result<(), Box<dyn std::error::Error>> {
	use super::InstanceHandle;
	use std::future::Future;
	super::block_on(async {
		let engine = Engine::default();
		let component = Component::from_file(
			&engine,
			concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/tests/dispatching/dependant_plugins_async/plugins/child-sync/root.wat"
			),
		)?;
		let instance = crate::Plugin::new(
			component,
			Context {
				table: ResourceTable::new(),
			},
		)
		.instantiate_async(&engine, &Linker::new(&engine))
		.await?;
		let handle = instance.handle();
		let reservation = super::CleanupReservation::new(&handle.inner.admission);
		assert!(handle.reserve_sync().is_none());
		let mut admission = Box::pin(handle.reserve_async());
		let waker = futures::task::noop_waker();
		let mut cx = std::task::Context::from_waker(&waker);
		assert!(admission.as_mut().poll(&mut cx).is_pending());
		drop(reservation);
		drop(admission.await?);
		assert!(handle.reserve_sync().is_some());
		Ok(())
	})
}

#[test]
fn unsupported_runtime_import_registration_reports_duplicate_names() -> Result<(), wasmtime::Error> {
	let engine = Engine::default();
	let mut linker = Linker::<Context>::new(&engine);
	linker
		.root()
		.instance("wasm-link:runtime/calls@0.5.0")?
		.func_wrap("cancellation-requested", |_, (): ()| Ok((false,)))?;
	assert!(super::add_runtime_imports(&mut linker, &std::sync::Arc::default()).is_err());
	Ok(())
}

#[test]
fn response_delivery_survives_driver_shutdown_and_closed_channels() {
	use futures::{FutureExt, StreamExt};
	use std::sync::Arc;
	for deliver in [false, true] {
		let (sender, mut receiver) = futures::channel::mpsc::unbounded::<super::Request>();
		let cleanup = Arc::new(super::CleanupQueue::default());
		let result_cleanup = cleanup.clone();
		let driver = async move {
			if let Some(request) = receiver.next().await {
				if deliver {
					request.respond(Ok(super::OwnedValue {
						value: Some(Val::U32(42)),
						cleanup: result_cleanup,
					}));
				} else {
					drop(request);
					futures::future::pending::<()>().await;
				}
			}
			"driver stopped".to_string()
		}
		.boxed()
		.shared();
		let handle = super::PluginDispatchHandle::<Context> {
			inner: Arc::new(super::InstanceInner {
				sender,
				driver,
				cleanup,
				admission: Arc::default(),
				_context: std::marker::PhantomData,
			}),
		};
		let context = super::CallContext::root(super::Cancellation::new(), false);
		let function = crate::Function::new(crate::FunctionKind::Freestanding, crate::ReturnKind::AssumeNoResources);
		let result =
			super::block_on(handle.dispatch_async(&context, "package", "interface", "function", &function, &[]));
		if deliver {
			assert!(matches!(result.map(super::OwnedValue::take), Ok(Val::U32(42))));
		} else {
			assert!(
				matches!(result, Err(CallError::RuntimeException(error)) if error.to_string().contains("store ended before producing a response"))
			);
		}
	}
}

#[test]
fn driver_shutdown_releases_waiters_for_admission() -> Result<(), Box<dyn std::error::Error>> {
	use futures::FutureExt;
	use std::sync::Arc;
	let (sender, _receiver) = futures::channel::mpsc::unbounded();
	let handle = super::PluginDispatchHandle::<Context> {
		inner: Arc::new(super::InstanceInner {
			sender,
			driver: futures::future::ready("driver failed".to_string()).boxed().shared(),
			cleanup: Arc::default(),
			admission: Arc::default(),
			_context: std::marker::PhantomData,
		}),
	};
	let _reservation = handle.reserve_sync().ok_or("cannot reserve plugin")?;
	assert!(
		matches!(super::block_on(handle.reserve_async()), Err(CallError::RuntimeException(error)) if error.to_string().contains("driver failed"))
	);
	Ok(())
}

#[test]
fn cleanup_reports_a_fuel_limiter_without_engine_fuel_support() -> Result<(), wasmtime::Error> {
	super::block_on(async {
		let engine = Engine::default();
		let component = Component::from_file(
			&engine,
			concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/tests/dispatching/dependant_plugins_async/plugins/child-sync/root.wat"
			),
		)?;
		let mut store = Store::new(
			&engine,
			Context {
				table: ResourceTable::new(),
			},
		);
		let instance = Linker::new(&engine).instantiate_async(&mut store, &component).await?;
		let resource = wasmtime::component::ResourceAny::try_from_resource(
			wasmtime::component::Resource::<u32>::new_own(7),
			&mut store,
		)?;
		let instance = super::PluginInstanceAsync::new(
			store,
			instance,
			std::collections::HashMap::new(),
			super::AsyncLinkage::new(),
			Some(Box::new(|_, _, _, _| 100)),
			None,
		);
		instance.inner.cleanup.enqueue(resource);
		let failure = instance.inner.driver.clone().await;
		assert!(failure.contains("fuel"), "{failure}");
		Ok(())
	})
}

#[test]
fn resource_traversal_handles_empty_and_populated_composites() -> Result<(), wasmtime::Error> {
    let mut store = Store::new(&Engine::default(), ());
    let resource = wasmtime::component::ResourceAny::try_from_resource(wasmtime::component::Resource::<u32>::new_own(7), &mut store)?;
    let nested = Val::Resource(resource);
    let values = [
        Val::List(vec![nested.clone()]), Val::FixedLengthList(vec![nested.clone()]),
        Val::Variant("empty".into(), None), Val::Variant("full".into(), Some(Box::new(nested.clone()))),
        Val::Option(None), Val::Option(Some(Box::new(nested.clone()))),
        Val::Result(Ok(None)), Val::Result(Err(None)),
        Val::Result(Ok(Some(Box::new(nested.clone())))), Val::Result(Err(Some(Box::new(nested)))),
    ];
    let mut visited = Vec::new();
    for value in values { super::visit_resources(value, &mut |r| visited.push(r)); }
    assert_eq!(visited, vec![resource; 6]);
    resource.resource_drop(&mut store)?;
    Ok(())
}

#[test]
fn a_closed_request_channel_reports_a_runtime_failure() {
    use futures::FutureExt;
    use std::sync::Arc;
    let (sender, receiver) = futures::channel::mpsc::unbounded();
    drop(receiver);
    let handle = super::PluginDispatchHandle::<Context> { inner:Arc::new(super::InstanceInner {
        sender, driver:futures::future::pending().boxed().shared(),
        cleanup:Arc::default(), admission:Arc::default(), _context:std::marker::PhantomData,
    }) };
    let context = super::CallContext::root(super::Cancellation::new(), false);
    let function = crate::Function::new(crate::FunctionKind::Freestanding, crate::ReturnKind::AssumeNoResources);
    let result = super::block_on(handle.dispatch_async(&context, "package", "interface", "function", &function, &[]));
    assert!(matches!(result, Err(CallError::RuntimeException(error)) if error.to_string().contains("store driver stopped")));
}
