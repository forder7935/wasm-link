use futures::lock::Mutex;
use std::sync::Arc;
use wasmtime::component::{Accessor, Val};
use wasmtime::{AsContextMut, StoreContextMut};

use crate::cardinality::Cardinality;
use crate::errors::RouteError;
use crate::{Binding, Function, FunctionKind, PluginContext, ReturnKind};

use super::resource_wrapper::ResourceWrapper;

pub(crate) struct DispatchTarget<'a> {
	package_name: &'a str,
	interface_name: &'a str,
	function_name: &'a str,
	function: &'a Function,
}

impl<'a> DispatchTarget<'a> {
	pub(crate) fn new(
		package_name: &'a str,
		interface_name: &'a str,
		function_name: &'a str,
		function: &'a Function,
	) -> Self {
		Self {
			package_name,
			interface_name,
			function_name,
			function,
		}
	}
}

/// Asynchronously dispatches a non-method function call to all plugins.
pub(crate) async fn dispatch_all_async<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: &Accessor<Ctx>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
	<<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>> as Cardinality<PluginId, Arc<Instance>>>::Rebind<Val>: Into<Val> + Send,
{
	debug_assert_eq!(target.function.kind(), FunctionKind::Freestanding);
	let deliveries = crate::plugin_instance::DeliveryBatch::default();
	let result = binding
		.plugins()
		.map_async(|plugin_id, plugin| async {
			Val::Result(
				match dispatch_of_async(context, ctx, plugin_id, plugin, target, data).await {
					Ok(val) => Ok(Some(Box::new(deliveries.hold(crate::plugin_instance::OwnedValue {
						value: Some(val),
						cleanup: context.cleanup.clone(),
					})))),
					Err(err) => Err(Some(Box::new(err.into()))),
				},
			)
		})
		.await
		.into();
	deliveries.delivered();
	result
}

/// Asynchronously dispatches a method call to the plugin owning its resource.
pub(crate) async fn dispatch_method_async<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: &Accessor<Ctx>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
{
	debug_assert_eq!(target.function.kind(), FunctionKind::Method);
	Val::Result(match route_method_async(binding, context, ctx, target, data).await {
		Ok(val) => Ok(Some(Box::new(val))),
		Err(err) => Err(Some(Box::new(err.into()))),
	})
}

/// Asynchronously implements a synchronous WIT import without blocking its host thread.
pub(crate) async fn dispatch_all_async_sync<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: StoreContextMut<'_, Ctx>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
	<<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>> as Cardinality<PluginId, Arc<Instance>>>::Rebind<Val>: Into<Val> + Send,
{
	debug_assert_eq!(target.function.kind(), FunctionKind::Freestanding);
	let ctx = Mutex::new(ctx);
	let deliveries = crate::plugin_instance::DeliveryBatch::default();
	let result = binding
		.plugins()
		.map_async(|plugin_id, plugin| async {
			Val::Result(
				match dispatch_of_async_sync(context, &ctx, plugin_id, plugin, target, data).await {
					Ok(val) => Ok(Some(Box::new(deliveries.hold(crate::plugin_instance::OwnedValue {
						value: Some(val),
						cleanup: context.cleanup.clone(),
					})))),
					Err(err) => Err(Some(Box::new(err.into()))),
				},
			)
		})
		.await
		.into();
	deliveries.delivered();
	result
}

/// Asynchronously implements a synchronous WIT method import.
pub(crate) async fn dispatch_method_async_sync<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: StoreContextMut<'_, Ctx>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Val
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
{
	debug_assert_eq!(target.function.kind(), FunctionKind::Method);
	let ctx = Mutex::new(ctx);
	Val::Result(
		match route_method_async_sync(binding, context, &ctx, target, data).await {
			Ok(val) => Ok(Some(Box::new(val))),
			Err(err) => Err(Some(Box::new(err.into()))),
		},
	)
}

async fn dispatch_of_async<PluginId, Ctx, Instance>(
	dispatch: &crate::plugin_instance::CallContext,
	ctx: &Accessor<Ctx>,
	plugin_id: PluginId,
	plugin: Arc<Instance>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Result<Val, RouteError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
{
	let instance = plugin.handle();
	let result = instance
		.dispatch_routed(
			dispatch,
			target.package_name,
			target.interface_name,
			target.function_name,
			target.function,
			data,
		)
		.await?;

	match target.function.return_kind() {
		ReturnKind::Void | ReturnKind::AssumeNoResources => Ok(result.take()),
		ReturnKind::MayContainResources => ctx.with(|mut access| {
			let mut store = access.as_context_mut();
			wrap_owned_resources(result, plugin_id, &mut store, &dispatch.cleanup)
		}),
	}
}

async fn dispatch_of_async_sync<PluginId, Ctx, Instance>(
	dispatch: &crate::plugin_instance::CallContext,
	ctx: &Mutex<StoreContextMut<'_, Ctx>>,
	plugin_id: PluginId,
	plugin: Arc<Instance>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Result<Val, RouteError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
{
	let instance = plugin.handle();
	let result = instance
		.dispatch_routed(
			dispatch,
			target.package_name,
			target.interface_name,
			target.function_name,
			target.function,
			data,
		)
		.await?;

	match target.function.return_kind() {
		ReturnKind::Void | ReturnKind::AssumeNoResources => Ok(result.take()),
		ReturnKind::MayContainResources => {
			let mut store = ctx.lock().await;
			wrap_owned_resources(result, plugin_id, &mut store, &dispatch.cleanup)
		}
	}
}

async fn route_method_async<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: &Accessor<Ctx>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Result<Val, RouteError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
{
	let handle = match data.first() {
		Some(Val::Resource(handle)) => Ok(*handle),
		_ => Err(RouteError::InvalidArgumentList),
	}?;
	let (plugin_id, resource_handle) = ctx.with(|mut access| {
		let mut store = access.as_context_mut();
		let resource = ResourceWrapper::<PluginId>::from_handle(handle, &mut store)?;
		Ok::<_, RouteError>((resource.plugin_id.clone(), resource.resource_handle))
	})?;
	let plugin = binding
		.plugins()
		.get(&plugin_id)
		.ok_or(RouteError::InvalidArgumentList)?
		.clone();

	let mut data = Vec::from(data);
	data[0] = Val::Resource(resource_handle);
	dispatch_of_async(context, ctx, plugin_id, plugin, target, &data).await
}

async fn route_method_async_sync<PluginId, Ctx, Plugins, Instance>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	context: &crate::plugin_instance::CallContext,
	ctx: &Mutex<StoreContextMut<'_, Ctx>>,
	target: &DispatchTarget<'_>,
	data: &[Val],
) -> Result<Val, RouteError>
where
	PluginId: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
	Ctx: PluginContext,
	Instance: crate::plugin_instance::InstanceHandle<Ctx> + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Send + Sync,
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>>: Cardinality<PluginId, Arc<Instance>>,
{
	let handle = match data.first() {
		Some(Val::Resource(handle)) => Ok(*handle),
		_ => Err(RouteError::InvalidArgumentList),
	}?;
	let (plugin_id, resource_handle) = {
		let mut store = ctx.lock().await;
		let resource = ResourceWrapper::<PluginId>::from_handle(handle, &mut store)?;
		(resource.plugin_id.clone(), resource.resource_handle)
	};
	let plugin = binding
		.plugins()
		.get(&plugin_id)
		.ok_or(RouteError::InvalidArgumentList)?
		.clone();
	let mut data = Vec::from(data);
	data[0] = Val::Resource(resource_handle);
	dispatch_of_async_sync(context, ctx, plugin_id, plugin, target, &data).await
}

fn wrap_resources<T, Id>(
	val: Val,
	plugin_id: Id,
	store: &mut StoreContextMut<T>,
	transaction: &mut WrapTransaction,
) -> Result<Val, RouteError>
where
	T: PluginContext,
	Id: Clone + Send + Sync + 'static,
{
	Ok(match val {
		Val::Bool(_)
		| Val::S8(_)
		| Val::S16(_)
		| Val::S32(_)
		| Val::S64(_)
		| Val::U8(_)
		| Val::U16(_)
		| Val::U32(_)
		| Val::U64(_)
		| Val::Float32(_)
		| Val::Float64(_)
		| Val::Char(_)
		| Val::String(_)
		| Val::Enum(_)
		| Val::Flags(_)
		| Val::Variant(_, Option::None)
		| Val::Option(None)
		| Val::Result(Ok(Option::None))
		| Val::Result(Err(Option::None)) => val,
		Val::List(list) => Val::List(
			list.into_iter()
				.map(|item| wrap_resources(item, plugin_id.clone(), store, transaction))
				.collect::<Result<_, _>>()?,
		),
		Val::FixedLengthList(list) => Val::FixedLengthList(
			list.into_iter()
				.map(|item| wrap_resources(item, plugin_id.clone(), store, transaction))
				.collect::<Result<_, _>>()?,
		),
		Val::Map(entries) => Val::Map(
			entries
				.into_iter()
				.map(|(key, value)| {
					Ok::<_, RouteError>((
						wrap_resources(key, plugin_id.clone(), store, transaction)?,
						wrap_resources(value, plugin_id.clone(), store, transaction)?,
					))
				})
				.collect::<Result<_, _>>()?,
		),
		Val::Record(entries) => Val::Record(
			entries
				.into_iter()
				.map(|(key, value)| {
					Ok::<_, RouteError>((key, wrap_resources(value, plugin_id.clone(), store, transaction)?))
				})
				.collect::<Result<_, _>>()?,
		),
		Val::Tuple(list) => Val::Tuple(
			list.into_iter()
				.map(|item| wrap_resources(item, plugin_id.clone(), store, transaction))
				.collect::<Result<_, _>>()?,
		),
		Val::Variant(variant, Some(data_box)) => Val::Variant(
			variant,
			Some(Box::new(wrap_resources(*data_box, plugin_id, store, transaction)?)),
		),
		Val::Option(Some(data_box)) => Val::Option(Some(Box::new(wrap_resources(
			*data_box,
			plugin_id,
			store,
			transaction,
		)?))),
		Val::Result(Ok(Some(data_box))) => Val::Result(Ok(Some(Box::new(wrap_resources(
			*data_box,
			plugin_id,
			store,
			transaction,
		)?)))),
		Val::Result(Err(Some(data_box))) => Val::Result(Err(Some(Box::new(wrap_resources(
			*data_box,
			plugin_id,
			store,
			transaction,
		)?)))),
		Val::Resource(handle) => {
			let wrapped = ResourceWrapper::new(plugin_id, handle)
				.with_cleanup(transaction.cleanup.clone(), transaction.armed.clone())
				.attach(store)?;
			transaction.created.push(wrapped);
			Val::Resource(wrapped)
		}
		Val::Future(_) => {
			return Err(RouteError::Call(crate::CallError::UnsupportedType(
				"future".to_string(),
			)))
		}
		Val::Stream(_) => {
			return Err(RouteError::Call(crate::CallError::UnsupportedType(
				"stream".to_string(),
			)))
		}
		Val::ErrorContext(_) => {
			return Err(RouteError::Call(crate::CallError::UnsupportedType(
				"error-context".to_string(),
			)))
		}
	})
}

struct WrapTransaction {
	cleanup: Arc<crate::plugin_instance::CleanupQueue>,
	armed: Arc<std::sync::atomic::AtomicBool>,
	created: Vec<wasmtime::component::ResourceAny>,
}
fn wrap_owned_resources<T: PluginContext, Id: Clone + Send + Sync + 'static>(
	value: crate::plugin_instance::OwnedValue,
	id: Id,
	store: &mut StoreContextMut<T>,
	caller_cleanup: &crate::plugin_instance::CleanupQueue,
) -> Result<Val, RouteError> {
	let mut transaction = WrapTransaction {
		cleanup: value.cleanup(),
		armed: Arc::default(),
		created: Vec::new(),
	};
	let result = wrap_resources(
		value.value().cloned().unwrap_or(Val::Option(None)),
		id,
		store,
		&mut transaction,
	);
	match result {
		Ok(result) => {
			value.take();
			transaction.armed.store(true, std::sync::atomic::Ordering::Release);
			Ok(result)
		}
		Err(error) => {
			for resource in transaction.created {
				caller_cleanup.enqueue(resource);
			}
			Err(error)
		}
	}
}

#[cfg(test)]
mod tests {
	include!("linker_tests.rs");
}
