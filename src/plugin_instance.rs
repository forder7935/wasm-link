use crate::errors::RouteError;
use crate::ready_queue::{ExecutionPathId, PluginKey, ReadyQueue};
use crate::{CallError, Function, PluginContext, Remap, ReturnKind, SyncCallError};
use futures::channel::{mpsc, oneshot};
use futures::future::{BoxFuture, FutureExt, Shared};
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::sync::{
	atomic::{AtomicBool, AtomicU64, Ordering},
	Arc, Mutex,
};
use std::task::{Poll, Wake, Waker};
use thiserror::Error;
use wasmtime::component::{Accessor, GuestTaskId, Instance, ResourceAny, Val};
use wasmtime::{AsContextMut, CallHook, Store, StoreContextMut};

type CallLimiter<Ctx> = Box<dyn FnMut(&mut StoreContextMut<'_, Ctx>, &str, &str, &Function) -> u64 + Send>;
struct FuelAccounting<'a, Ctx: 'static> {
	limiter: &'a mut Option<CallLimiter<Ctx>>,
	budgets: Arc<Mutex<HashMap<GuestTaskId, u64>>>,
}

/// A plugin with synchronous public dispatch. Its internal dependency imports
/// can suspend when this instance is used in an asynchronous tree.
pub struct PluginInstanceSync<Ctx: 'static> {
	inner: Arc<InstanceInner<Ctx>>,
}
/// A plugin with asynchronous public dispatch. Cloned bindings share its store.
pub struct PluginInstanceAsync<Ctx: 'static> {
	inner: Arc<InstanceInner<Ctx>>,
}
pub(crate) trait InstanceHandle<Ctx: 'static> {
	fn handle(&self) -> PluginDispatchHandle<Ctx>;
}
pub(crate) struct PluginDispatchHandle<Ctx: 'static> {
	inner: Arc<InstanceInner<Ctx>>,
}
impl<Ctx> Clone for PluginDispatchHandle<Ctx> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}
impl<Ctx> InstanceHandle<Ctx> for PluginInstanceSync<Ctx> {
	fn handle(&self) -> PluginDispatchHandle<Ctx> {
		PluginDispatchHandle {
			inner: self.inner.clone(),
		}
	}
}
impl<Ctx> InstanceHandle<Ctx> for PluginInstanceAsync<Ctx> {
	fn handle(&self) -> PluginDispatchHandle<Ctx> {
		PluginDispatchHandle {
			inner: self.inner.clone(),
		}
	}
}
impl<Ctx> std::fmt::Debug for PluginInstanceSync<Ctx> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PluginInstanceSync").finish_non_exhaustive()
	}
}
impl<Ctx> std::fmt::Debug for PluginInstanceAsync<Ctx> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PluginInstanceAsync").finish_non_exhaustive()
	}
}

#[derive(Clone, Default)]
pub(crate) struct Cancellation {
	flag: Arc<AtomicBool>,
	parent: Option<Arc<Cancellation>>,
}
impl Cancellation {
	pub(crate) fn new() -> Self {
		Self::default()
	}
	fn requested(&self) -> bool {
		self.flag.load(Ordering::Acquire) || self.parent.as_ref().is_some_and(|parent| parent.requested())
	}
}
pub(crate) struct CancelOnDrop {
	cancellation: Cancellation,
	armed: bool,
}
impl CancelOnDrop {
	pub(crate) fn new(cancellation: Cancellation) -> Self {
		Self {
			cancellation,
			armed: true,
		}
	}
	pub(crate) fn complete(mut self) {
		self.armed = false;
	}
}
impl Drop for CancelOnDrop {
	fn drop(&mut self) {
		if self.armed {
			self.cancellation.flag.store(true, Ordering::Release);
		}
	}
}
#[derive(Clone)]
pub(crate) struct CallContext {
	cancellation: Cancellation,
	synchronous: bool,
	pub(crate) caller: PluginKey,
	path: ExecutionPathId,
	id: u64,
	pub(crate) cleanup: Arc<CleanupQueue>,
}
static NEXT_CALL: AtomicU64 = AtomicU64::new(1);
impl CallContext {
	pub(crate) fn root(cancellation: Cancellation, synchronous: bool) -> Self {
		Self {
			cancellation,
			synchronous,
			caller: PluginKey(0),
			path: ExecutionPathId(0),
			id: 0,
			cleanup: Arc::default(),
		}
	}
	fn child(&self, caller: PluginKey) -> Self {
		Self {
			id: NEXT_CALL.fetch_add(1, Ordering::Relaxed),
			caller,
			cancellation: Cancellation {
				flag: Arc::default(),
				parent: Some(Arc::new(self.cancellation.clone())),
			},
			path: ExecutionPathId(self.id),
			..self.clone()
		}
	}
}
pub(crate) type ActiveCalls = Arc<Mutex<HashMap<GuestTaskId, CallContext>>>;
pub(crate) struct AsyncLinkage {
	active_calls: ActiveCalls,
}
impl AsyncLinkage {
	pub(crate) fn new() -> Self {
		Self {
			active_calls: Arc::default(),
		}
	}
	pub(crate) fn active_calls(&self) -> &ActiveCalls {
		&self.active_calls
	}
}
#[derive(Debug, Error)]
pub(crate) enum ContextError {
	#[error("runtime import called outside a plugin invocation")]
	MissingTask,
	#[error("runtime import called by an unregistered task")]
	UnknownTask,
}
pub(crate) fn active_call(calls: &ActiveCalls, task: GuestTaskId) -> Result<CallContext, ContextError> {
	calls
		.lock()
		.unwrap_or_else(std::sync::PoisonError::into_inner)
		.get(&task)
		.cloned()
		.ok_or(ContextError::UnknownTask)
}

// Keep the blocking adapter local: unlike an executor's block_on, this permits
// synchronous host callbacks to enter an independent plugin on the same thread.
pub(crate) fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
	struct ThreadWake(std::thread::Thread);
	impl Wake for ThreadWake {
		fn wake(self: Arc<Self>) {
			self.0.unpark();
		}
		fn wake_by_ref(self: &Arc<Self>) {
			self.0.unpark();
		}
	}
	let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
	let mut cx = std::task::Context::from_waker(&waker);
	let mut future = std::pin::pin!(future);
	loop {
		match future.as_mut().poll(&mut cx) {
			Poll::Ready(result) => return result,
			Poll::Pending => std::thread::park(),
		}
	}
}

#[derive(Default)]
struct Admission {
	cleaning: bool,
	exclusive: bool,
	count: usize,
	waiters: Vec<Waker>,
}
struct Permit {
	admission: Arc<Mutex<Admission>>,
	exclusive: bool,
}
impl Drop for Permit {
	fn drop(&mut self) {
		let waiters = {
			let mut state = self.admission.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			state.count -= 1;
			if self.exclusive {
				state.exclusive = false;
			}
			std::mem::take(&mut state.waiters)
		};
		for waker in waiters {
			waker.wake();
		}
	}
}

struct CleanupReservation(Arc<Mutex<Admission>>);
impl CleanupReservation {
	fn new(admission: &Arc<Mutex<Admission>>) -> Self {
		admission
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
			.cleaning = true;
		Self(admission.clone())
	}
}
impl Drop for CleanupReservation {
	fn drop(&mut self) {
		let waiters = {
			let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			state.cleaning = false;
			std::mem::take(&mut state.waiters)
		};
		for waker in waiters {
			waker.wake();
		}
	}
}

#[derive(Default)]
pub(crate) struct CleanupQueue {
	resources: Mutex<Vec<ResourceAny>>,
	waker: futures::task::AtomicWaker,
}
impl CleanupQueue {
	pub(crate) fn enqueue(&self, resource: ResourceAny) {
		self.resources
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
			.push(resource);
		self.waker.wake();
	}
}
/// Owns a result until delivery, including while it sits in a channel or `join_all`.
pub(crate) struct OwnedValue {
	pub(crate) value: Option<Val>,
	pub(crate) cleanup: Arc<CleanupQueue>,
}
impl OwnedValue {
	pub(crate) fn value(&self) -> Option<&Val> {
		self.value.as_ref()
	}
	pub(crate) fn cleanup(&self) -> Arc<CleanupQueue> {
		self.cleanup.clone()
	}
	pub(crate) fn take(mut self) -> Val {
		self.value.take().unwrap_or(Val::Option(None))
	}
}
impl Drop for OwnedValue {
	fn drop(&mut self) {
		if let Some(value) = self.value.take() {
			visit_resources(value, &mut |resource| self.cleanup.enqueue(resource));
		}
	}
}
fn visit_resources(value: Val, visit: &mut impl FnMut(ResourceAny)) {
	match value {
		Val::Resource(r) => visit(r),
		Val::List(v) | Val::FixedLengthList(v) | Val::Tuple(v) => {
			for v in v {
				visit_resources(v, visit);
			}
		}
		Val::Map(v) => {
			for (k, v) in v {
				visit_resources(k, visit);
				visit_resources(v, visit);
			}
		}
		Val::Record(v) => {
			for (_, v) in v {
				visit_resources(v, visit);
			}
		}
		Val::Variant(_, Some(v)) | Val::Option(Some(v)) | Val::Result(Ok(Some(v))) | Val::Result(Err(Some(v))) => {
			visit_resources(*v, visit);
		}
		_ => {}
	}
}
// Keeps ownership of all completed members of a public fan-out until the whole
// result is delivered. Dropping join_all must not lose completed resource handles.
#[derive(Default)]
pub(crate) struct DeliveryBatch(Mutex<Vec<OwnedValue>>);
impl DeliveryBatch {
	pub(crate) fn hold(&self, value: OwnedValue) -> Val {
		let raw = value.value().cloned().unwrap_or(Val::Option(None));
		self.0
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
			.push(value);
		raw
	}
	pub(crate) fn delivered(&self) {
		for value in std::mem::take(&mut *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)) {
			value.take();
		}
	}
}

struct InstanceInner<Ctx: 'static> {
	sender: mpsc::UnboundedSender<Request>,
	driver: Shared<BoxFuture<'static, String>>,
	admission: Arc<Mutex<Admission>>,
	cleanup: Arc<CleanupQueue>,
	_context: std::marker::PhantomData<fn() -> Ctx>,
}
struct Request {
	package_name: String,
	interface_name: String,
	function_name: String,
	function: Function,
	data: Vec<Val>,
	response: Option<oneshot::Sender<Result<OwnedValue, CallError>>>,
	active: CallContext,
	requester: PluginKey,
	permit: Option<Permit>,
}
impl Request {
	fn cancelled(&self) -> bool {
		self.response.as_ref().is_none_or(oneshot::Sender::is_canceled)
	}
	fn respond(mut self, result: Result<OwnedValue, CallError>) {
		if let Some(response) = self.response.take() {
			let _ = response.send(result);
		}
	}
}
struct PluginState<Ctx: 'static> {
	store: Store<Ctx>,
	instance: Instance,
	interface_remaps: HashMap<String, Remap>,
	fuel_limiter: Option<CallLimiter<Ctx>>,
	epoch_limiter: Option<CallLimiter<Ctx>>,
}

impl<Ctx: PluginContext + 'static> PluginInstanceAsync<Ctx> {
	pub(crate) fn new(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		linkage: AsyncLinkage,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
	) -> Self {
		let state = PluginState {
			store,
			instance,
			interface_remaps,
			fuel_limiter,
			epoch_limiter,
		};
		let (sender, receiver) = mpsc::unbounded();
		let cleanup = Arc::new(CleanupQueue::default());
		let driver_cleanup = cleanup.clone();
		let admission = Arc::default();
		let driver_admission = Arc::clone(&admission);
		let driver =
			async move { run_store(state, receiver, linkage.active_calls, driver_cleanup, driver_admission).await }
				.boxed()
				.shared();
		Self {
			inner: Arc::new(InstanceInner {
				sender,
				driver,
				cleanup,
				admission,
				_context: std::marker::PhantomData,
			}),
		}
	}
	pub(crate) fn into_sync(self) -> PluginInstanceSync<Ctx> {
		PluginInstanceSync { inner: self.inner }
	}
}
impl<Ctx> From<PluginInstanceSync<Ctx>> for PluginInstanceAsync<Ctx> {
	fn from(instance: PluginInstanceSync<Ctx>) -> Self {
		Self { inner: instance.inner }
	}
}
impl<Ctx: PluginContext + 'static> PluginInstanceSync<Ctx> {
	pub(crate) fn dispatch(
		&self,
		package: &str,
		interface: &str,
		name: &str,
		function: &Function,
		args: &[Val],
	) -> Result<Val, SyncCallError> {
		let handle = self.handle();
		let permit = handle.reserve_sync().ok_or(SyncCallError::PluginBusy)?;
		let cancellation = Cancellation::new();
		let guard = CancelOnDrop::new(cancellation.clone());
		let context = CallContext::root(cancellation, true);
		let result = block_on(handle.invoke(&context, package, interface, name, function, args, permit))
			.map(OwnedValue::take)
			.map_err(Into::into);
		guard.complete();
		result
	}
}
impl<Ctx: PluginContext + 'static> PluginDispatchHandle<Ctx> {
	pub(crate) fn release(&self, value: Val) {
		visit_resources(value, &mut |resource| self.inner.cleanup.enqueue(resource));
	}
	fn reserve_sync(&self) -> Option<Permit> {
		let mut state = self
			.inner
			.admission
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		if state.count != 0 || state.cleaning {
			return None;
		}
		state.exclusive = true;
		state.count = 1;
		Some(Permit {
			admission: self.inner.admission.clone(),
			exclusive: true,
		})
	}
	async fn reserve_async(&self) -> Permit {
		futures::future::poll_fn(|cx| {
			let mut state = self
				.inner
				.admission
				.lock()
				.unwrap_or_else(std::sync::PoisonError::into_inner);
			if state.exclusive || state.cleaning {
				if !state.waiters.iter().any(|w| w.will_wake(cx.waker())) {
					state.waiters.push(cx.waker().clone());
				}
				return Poll::Pending;
			}
			state.count += 1;
			Poll::Ready(Permit {
				admission: self.inner.admission.clone(),
				exclusive: false,
			})
		})
		.await
	}
	pub(crate) async fn dispatch_async(
		&self,
		context: &CallContext,
		package: &str,
		interface: &str,
		name: &str,
		function: &Function,
		args: &[Val],
	) -> Result<OwnedValue, CallError> {
		let permit = self.reserve_async().await;
		self.invoke(context, package, interface, name, function, args, permit)
			.await
	}
	pub(crate) async fn dispatch_routed(
		&self,
		context: &CallContext,
		package: &str,
		interface: &str,
		name: &str,
		function: &Function,
		args: &[Val],
	) -> Result<OwnedValue, RouteError> {
		let permit = if context.synchronous {
			self.reserve_sync().ok_or(RouteError::PluginBusy)?
		} else {
			self.reserve_async().await
		};
		Ok(self
			.invoke(context, package, interface, name, function, args, permit)
			.await?)
	}
	#[allow(clippy::too_many_arguments)]
	async fn invoke(
		&self,
		context: &CallContext,
		package: &str,
		interface: &str,
		name: &str,
		function: &Function,
		args: &[Val],
		permit: Permit,
	) -> Result<OwnedValue, CallError> {
		ensure_supported_values(args)?;
		let (response, result) = oneshot::channel();
		let active = context.child(PluginKey(Arc::as_ptr(&self.inner) as usize));
		let guard = CancelOnDrop::new(active.cancellation.clone());
		let request = Request {
			package_name: package.into(),
			interface_name: interface.into(),
			function_name: name.into(),
			function: function.clone(),
			data: args.to_vec(),
			response: Some(response),
			requester: context.caller,
			active,
			permit: Some(permit),
		};
		if let Some(error) = self.inner.driver.peek() {
			return Err(store_error(error));
		}
		self.inner
			.sender
			.unbounded_send(request)
			.map_err(|_| store_error("store driver stopped"))?;
		let driver = self.inner.driver.clone();
		futures::pin_mut!(driver, result);
		let result = match futures::future::select(result, driver).await {
			futures::future::Either::Left((result, _)) => result.map_err(|_| {
				store_error(
					self.inner
						.driver
						.peek()
						.map_or("store ended before producing a response", String::as_str),
				)
			})?,
			futures::future::Either::Right((error, result)) => match result.now_or_never() {
				Some(Ok(result)) => result,
				_ => Err(store_error(&error)),
			},
		};
		guard.complete();
		result
	}
}
fn store_error(message: &str) -> CallError {
	#[derive(Debug, Error)]
	#[error("plugin store failed: {0}")]
	struct StoreFailed(String);
	CallError::RuntimeException(StoreFailed(message.into()).into())
}
// Keep admission, quiescence, and cleanup transitions together so their ordering
// remains visible: destructors must never run alongside admitted guest work.
#[allow(clippy::too_many_lines)]
async fn run_store<Ctx: PluginContext + 'static>(
	mut state: PluginState<Ctx>,
	mut receiver: mpsc::UnboundedReceiver<Request>,
	active_calls: ActiveCalls,
	cleanup: Arc<CleanupQueue>,
	admission: Arc<Mutex<Admission>>,
) -> String {
	let budgets = Arc::new(Mutex::new(HashMap::<GuestTaskId, u64>::new()));
	if state.fuel_limiter.is_some() {
		let budgets = budgets.clone();
		state.store.call_hook(move |mut store, hook| {
			let Some(task) = store.async_call_stack()?.last() else {
				return Ok(());
			};
			let mut budgets = budgets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			if let Some(remaining) = budgets.get_mut(&task) {
				match hook {
					CallHook::CallingWasm | CallHook::ReturningFromHost => store.set_fuel(*remaining)?,
					CallHook::ReturningFromWasm | CallHook::CallingHost => *remaining = store.get_fuel()?,
				}
			}
			Ok(())
		});
	}
	let mut fuel = FuelAccounting {
		limiter: &mut state.fuel_limiter,
		budgets,
	};
	let epoch = &mut state.epoch_limiter;
	let instance = state.instance;
	let remaps = &state.interface_remaps;
	let retired = Arc::new(Mutex::new(Vec::<Permit>::new()));
	let mut ready = ReadyQueue::default();
	let result = loop {
		let result = state
			.store
			.run_concurrent(async |accessor| -> Result<Option<Vec<ResourceAny>>, wasmtime::Error> {
				let mut calls = FuturesUnordered::<BoxFuture<'_, ()>>::new();
				loop {
					// A component may keep executing after task.return. Retain context,
					// fuel accounting, and admission until the store reports quiescence.
					let quiescent =
						futures::future::poll_fn(|cx| Poll::Ready(accessor.poll_no_interesting_tasks(cx).is_ready()))
							.await;
					if quiescent {
						retired
							.lock()
							.unwrap_or_else(std::sync::PoisonError::into_inner)
							.clear();
						active_calls
							.lock()
							.unwrap_or_else(std::sync::PoisonError::into_inner)
							.clear();
						fuel.budgets
							.lock()
							.unwrap_or_else(std::sync::PoisonError::into_inner)
							.clear();
					}
					if quiescent && calls.is_empty() {
						let resources = std::mem::take(
							&mut *cleanup
								.resources
								.lock()
								.unwrap_or_else(std::sync::PoisonError::into_inner),
						);
						if !resources.is_empty() {
							return Ok(Some(resources));
						}
					}
					// Bound inbox draining so a hot producer cannot prevent existing work
					// or cleanup from being polled.
					for _ in 0..64 {
						let Ok(request) = receiver.try_recv() else {
							break;
						};
						ready.enqueue(request.requester, request.active.path, request);
					}
					if epoch.is_none() || (calls.is_empty() && quiescent) {
						if let Some(request) = ready.dequeue() {
							if !request.cancelled() {
								start_call(
									accessor,
									instance,
									remaps,
									&active_calls,
									&mut fuel,
									epoch,
									request,
									cleanup.clone(),
									retired.clone(),
									&mut calls,
								);
							}
							// Poll already-admitted calls before accepting another request.
							let _ = calls.next().now_or_never();
							let mut yielded = false;
							futures::future::poll_fn(|cx| {
								if yielded {
									Poll::Ready(())
								} else {
									yielded = true;
									cx.waker().wake_by_ref();
									Poll::Pending
								}
							})
							.await;
							continue;
						}
					}
					let calls_empty = calls.is_empty();
					let clean = futures::future::poll_fn(|cx| {
						cleanup.waker.register(cx.waker());
						let mut resources = cleanup
							.resources
							.lock()
							.unwrap_or_else(std::sync::PoisonError::into_inner);
						if resources.is_empty() || !calls_empty || accessor.poll_no_interesting_tasks(cx).is_pending() {
							Poll::Pending
						} else {
							Poll::Ready(std::mem::take(&mut *resources))
						}
					})
					.fuse();
					let incoming = receiver.next().fuse();
					let completed = async {
						if calls.is_empty() {
							futures::future::pending::<()>().await;
						}
						calls.next().await;
					}
					.fuse();
					futures::pin_mut!(clean, incoming, completed);
					futures::select_biased! {
						resources = clean => return Ok(Some(resources)),
						() = completed => {},
						incoming = incoming => match incoming {
							Some(request) => ready.enqueue(request.requester, request.active.path, request),
							None => return Ok(None),
						},
					}
				}
			})
			.await;
		match result {
			Ok(Ok(Some(resources))) => {
				let _reservation = CleanupReservation::new(&admission);
				let metadata = Function::new(crate::FunctionKind::Freestanding, ReturnKind::Void);
				for resource in resources {
					let mut store = state.store.as_context_mut();
					if let Some(limiter) = fuel.limiter.as_mut() {
						let budget = limiter(&mut store, "wasm-link:runtime/cleanup", "resource-drop", &metadata);
						if let Err(error) = store.set_fuel(budget) {
							return format!("{error:#}");
						}
					}
					if let Some(limiter) = epoch.as_mut() {
						let deadline = limiter(&mut store, "wasm-link:runtime/cleanup", "resource-drop", &metadata);
						store.set_epoch_deadline(deadline);
					}
					if let Err(error) = resource.resource_drop_async(&mut store).await {
						return format!("resource cleanup failed: {error:#}");
					}
				}
			}
			other => break other,
		}
	};
	active_calls
		.lock()
		.unwrap_or_else(std::sync::PoisonError::into_inner)
		.clear();
	match result {
		Ok(Ok(_)) => "store stopped".into(),
		Ok(Err(e)) | Err(e) => format!("{e:#}"),
	}
}

#[allow(clippy::too_many_arguments)]
fn start_call<'a, Ctx: PluginContext + 'static>(
	accessor: &'a Accessor<Ctx>,
	instance: Instance,
	remaps: &HashMap<String, Remap>,
	active_calls: &ActiveCalls,
	fuel: &mut FuelAccounting<'_, Ctx>,
	epoch: &mut Option<CallLimiter<Ctx>>,
	mut request: Request,
	cleanup: Arc<CleanupQueue>,
	retired: Arc<Mutex<Vec<Permit>>>,
	calls: &mut FuturesUnordered<BoxFuture<'a, ()>>,
) {
	request.active.cleanup = cleanup.clone();
	let (interface, name) = resolve_export(
		remaps,
		&request.package_name,
		&request.interface_name,
		&request.function_name,
	);
	let prepared = accessor.with(|mut access| {
		let mut store = access.as_context_mut();
		let interface_index = instance
			.get_export_index(&mut store, None, &interface)
			.ok_or_else(|| CallError::InvalidInterfacePath(interface.clone()))?;
		let function_index = instance
			.get_export_index(&mut store, Some(&interface_index), &name)
			.ok_or_else(|| CallError::InvalidFunction(format!("{interface}:{name}")))?;
		let function = instance
			.get_func(&mut store, function_index)
			.ok_or_else(|| CallError::InvalidFunction(format!("{interface}:{name}")))?;
		let budget = fuel
			.limiter
			.as_mut()
			.map(|limiter| {
				store.get_fuel().map_err(CallError::RuntimeException)?;
				Ok::<_, CallError>(limiter(
					&mut store,
					&format!("{}/{}", request.package_name, request.interface_name),
					&request.function_name,
					&request.function,
				))
			})
			.transpose()?;
		if let Some(limiter) = epoch.as_mut() {
			let ticks = limiter(
				&mut store,
				&format!("{}/{}", request.package_name, request.interface_name),
				&request.function_name,
				&request.function,
			);
			store.set_epoch_deadline(ticks);
		}
		Ok::<_, CallError>((function, budget))
	});
	let (function, budget) = match prepared {
		Ok(v) => v,
		Err(e) => {
			request.respond(Err(e));
			return;
		}
	};
	let active_calls = active_calls.clone();
	let budgets = fuel.budgets.clone();
	calls.push(
		async move {
			let mut results = if request.function.return_kind() == ReturnKind::Void {
				Vec::new()
			} else {
				vec![Val::Option(None)]
			};
			let call = accessor.with(|mut access| {
				function.start_call_concurrent(access.as_context_mut(), &request.data, &mut results)
			});
			let call = match call {
				Ok(call) => call,
				Err(e) => {
					request.respond(Err(CallError::RuntimeException(e)));
					return;
				}
			};
			let task = call.task();
			if let Some(budget) = budget {
				budgets
					.lock()
					.unwrap_or_else(std::sync::PoisonError::into_inner)
					.insert(task, budget);
			}
			active_calls
				.lock()
				.unwrap_or_else(std::sync::PoisonError::into_inner)
				.insert(task, request.active.clone());
			let result = function.finish_call_concurrent(accessor, call).await;
			if let Some(permit) = request.permit.take() {
				retired
					.lock()
					.unwrap_or_else(std::sync::PoisonError::into_inner)
					.push(permit);
			}
			let result = result.map_err(CallError::RuntimeException).and_then(|()| {
				let value = OwnedValue {
					value: Some(results.pop().unwrap_or(Val::Option(None))),
					cleanup,
				};
				if let Some(raw) = value.value() {
					ensure_supported_value(raw)?;
				}
				Ok(value)
			});
			request.respond(result);
		}
		.boxed(),
	);
}
fn resolve_export(
	interface_remaps: &HashMap<String, Remap>,
	package_name: &str,
	interface_name: &str,
	function_name: &str,
) -> (String, String) {
	match interface_remaps.get(interface_name) {
		Some(remap) => (
			format!("{}/{}", package_name, remap.interface_name(interface_name)),
			remap.item_name(function_name).to_string(),
		),
		None => (format!("{package_name}/{interface_name}"), function_name.to_string()),
	}
}

fn ensure_supported_values(values: &[Val]) -> Result<(), CallError> {
	values.iter().try_for_each(ensure_supported_value)
}

fn ensure_supported_value(value: &Val) -> Result<(), CallError> {
	match value {
		Val::List(values) | Val::FixedLengthList(values) | Val::Tuple(values) => ensure_supported_values(values),
		Val::Map(values) => values.iter().try_for_each(|(key, value)| {
			ensure_supported_value(key)?;
			ensure_supported_value(value)
		}),
		Val::Record(values) => values.iter().try_for_each(|(_, value)| ensure_supported_value(value)),
		Val::Variant(_, Some(value))
		| Val::Option(Some(value))
		| Val::Result(Ok(Some(value)))
		| Val::Result(Err(Some(value))) => ensure_supported_value(value),
		Val::Future(_) => Err(CallError::UnsupportedType("future".to_string())),
		Val::Stream(_) => Err(CallError::UnsupportedType("stream".to_string())),
		Val::ErrorContext(_) => Err(CallError::UnsupportedType("error-context".to_string())),
		_ => Ok(()),
	}
}

pub(crate) fn add_runtime_imports<Ctx: PluginContext + 'static>(
	linker: &mut wasmtime::component::Linker<Ctx>,
	calls: &ActiveCalls,
) -> wasmtime::Result<()> {
	let mut root = linker.root();
	let mut interface = root.instance("wasm-link:runtime/calls@0.5.0")?;
	let current = calls.clone();
	interface.func_wrap("current-call", move |mut store: StoreContextMut<Ctx>, (): ()| {
		let task = store.async_call_stack()?.last().ok_or(ContextError::MissingTask)?;
		Ok((active_call(&current, task)?.id,))
	})?;
	let current = calls.clone();
	interface.func_wrap(
		"cancellation-requested",
		move |mut store: StoreContextMut<Ctx>, (): ()| {
			let task = store.async_call_stack()?.last().ok_or(ContextError::MissingTask)?;
			Ok((active_call(&current, task)?.cancellation.requested(),))
		},
	)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	include!("plugin_instance_tests.rs");
}
