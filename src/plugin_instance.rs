use std::collections::{ HashMap, VecDeque };
use std::sync::{ Arc, Weak };
use std::sync::atomic::{ AtomicU64, Ordering };
use futures::future::BoxFuture ;
use futures::lock::Mutex ;
use futures::task::{ FutureObj, Spawn };
use thiserror::Error ;
use wasmtime::component::{ Instance, Val };
use wasmtime::Store ;

use crate::{ Function, PluginContext, Remap, ReturnKind };
use crate::resource_wrapper::{ ResourceCreationError, ResourceReceiveError };

type CallLimiter<Ctx> = Box<dyn FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send>;

const MAX_CALLER_CALLS: usize = 1_024 ;
const MAX_CALLER_BYTES: usize = 64 * 1_024 * 1_024 ;
const MAX_DESTINATION_CALLS: usize = 4_096 ;
const MAX_DESTINATION_BYTES: usize = 256 * 1_024 * 1_024 ;

static NEXT_CALLER_ID: AtomicU64 = AtomicU64::new( 1 );

#[derive( Clone )]
pub(crate) struct Caller {
	id: u64,
	budget: Arc<std::sync::Mutex<Budget>>,
}

#[derive( Default )]
struct Budget {
	calls: usize,
	bytes: usize,
}

impl Caller {
	pub(crate) fn new() -> Self {
		Self {
			id: NEXT_CALLER_ID.fetch_add( 1, Ordering::Relaxed ),
			budget: Arc::new( std::sync::Mutex::new( Budget::default() )),
		}
	}
}


/// A synchronously instantiated plugin, ready for synchronous dispatch.
///
/// Created by calling [`Plugin::instantiate`]( crate::Plugin::instantiate ),
/// or [`Plugin::link`]( crate::Plugin::link ). Concurrent calls wait at a FIFO
/// admission gate so shared synchronous dependencies do not fail when busy.
pub struct PluginInstanceSync<Ctx: 'static> {
	dispatcher: Arc<SyncDispatcher<Ctx>>,
}

impl<Ctx: 'static> Clone for PluginInstanceSync<Ctx> {
	fn clone( &self ) -> Self { Self { dispatcher: Arc::clone( &self.dispatcher )}}
}

struct SyncDispatcher<Ctx: 'static> {
	state: std::sync::Mutex<PluginState<Ctx>>,
	admission: std::sync::Mutex<SyncQueue>,
	changed: std::sync::Condvar,
}

#[derive( Default )]
struct SyncQueue {
	waiting: VecDeque<u64>,
	active: Option<u64>,
	next_ticket: u64,
}

struct SyncPermit<'a, Ctx: 'static> {
	dispatcher: &'a SyncDispatcher<Ctx>,
}

/// An asynchronously instantiated plugin, ready for asynchronous dispatch.
///
/// Created by calling [`Plugin::instantiate_async`]( crate::Plugin::instantiate_async )
/// or [`Plugin::link_async`]( crate::Plugin::link_async ). Calls are submitted to the
/// executor supplied during instantiation. Each destination services one call at a
/// time using caller-aware round-robin ordering. The plugin's Wasmtime [`Store`]
/// remains independent and is serialized by the destination's drain task.
pub struct PluginInstanceAsync<Ctx: 'static> {
	dispatcher: Arc<Dispatcher<Ctx>>,
}

impl<Ctx: 'static> Clone for PluginInstanceAsync<Ctx> {
	fn clone( &self ) -> Self { Self { dispatcher: Arc::clone( &self.dispatcher )}}
}

struct Dispatcher<Ctx: 'static> {
	state: Arc<Mutex<PluginState<Ctx>>>,
	executor: Arc<dyn Spawn + Send + Sync>,
	queue: std::sync::Mutex<DispatchQueue>,
}

struct DrainGuard<Ctx: PluginContext + 'static> {
	dispatcher: Arc<Dispatcher<Ctx>>,
}

#[derive( Default )]
struct DispatchQueue {
	callers: HashMap<u64, CallerQueue>,
	ready: VecDeque<u64>,
	calls: usize,
	bytes: usize,
	draining: bool,
}

#[derive( Default )]
struct CallerQueue {
	calls: VecDeque<PendingCall>,
	ready: bool,
}

struct PendingCall {
	package_name: String,
	interface_name: String,
	function_name: String,
	function: Function,
	data: Vec<Val>,
	response: futures::channel::oneshot::Sender<Result<Val, DispatchError>>,
	_reservation: Reservation,
}

struct Reservation {
	caller: Caller,
	destination: Weak<dyn DestinationBudget>,
	bytes: usize,
}

trait DestinationBudget: Send + Sync {
	fn release( &self, bytes: usize );
}

struct PluginState<Ctx: 'static> {
	store: Store<Ctx>,
	instance: Instance,
	interface_remaps: HashMap<String, Remap>,
	fuel_limiter: Option<CallLimiter<Ctx>>,
	epoch_limiter: Option<CallLimiter<Ctx>>,
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for PluginInstanceSync<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		let state = lock_unpoisoned( &self.dispatcher.state );
		f.debug_struct( "PluginInstanceSync" )
			.field( "data", &state.store.data() )
			.field( "store", &state.store )
			.field( "interface_remaps", &state.interface_remaps )
			.field( "fuel_limiter", &state.fuel_limiter.as_ref().map(| _ | "<closure>" ))
			.field( "epoch_limiter", &state.epoch_limiter.as_ref().map(| _ | "<closure>" ))
			.finish_non_exhaustive()
	}
}

impl<Ctx: 'static> std::fmt::Debug for PluginInstanceAsync<Ctx> {
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::result::Result<(), std::fmt::Error> {
		f.debug_struct( "PluginInstanceAsync" )
			.field( "state", &"<serialized store>" )
			.field( "executor", &"<executor>" )
			.field( "dispatch_queue", &"<caller-aware round-robin>" )
			.finish_non_exhaustive()
	}
}

/// Errors that can occur when dispatching a function call to plugins.
///
/// Returned inside a cardinality wrapper from
/// [`Binding::dispatch`]( crate::binding::Binding::dispatch )
/// when a function call fails at runtime.
#[derive( Error, Debug )]
pub enum DispatchError {
	/// The specified interface path doesn't match any known interface.
	#[error( "Invalid Interface Path: {0}" )] InvalidInterfacePath( String ),
	/// The specified function doesn't exist on the interface.
	#[error( "Invalid Function: {0}" )] InvalidFunction( String ),
	/// Function was expected to return a value but didn't.
	#[error( "Missing Response" )] MissingResponse,
	/// The WASM function threw an exception during execution.
	#[error( "Runtime Exception" )] RuntimeException( wasmtime::Error ),
	/// The provided arguments don't match the function signature.
	#[error( "Invalid Argument List" )] InvalidArgumentList,
	/// Async types (`Future`, `Stream`, `ErrorContext`) are not yet supported for cross-plugin transfer.
	#[error( "Unsupported type: {0}" )] UnsupportedType( String ),
	/// The executor supplied for an async plugin rejected a dispatch task.
	#[error( "Async executor unavailable" )] ExecutorUnavailable,
	/// The caller or destination async dispatch queue reached a count or byte limit.
	#[error( "Dispatch queue full" )] DispatchQueueFull,
	/// Failed to create a resource handle for cross-plugin transfer.
	#[error( "Resource Create Error: {0}" )] ResourceCreationError( #[from] ResourceCreationError ),
	/// Failed to receive a resource handle from another plugin.
	#[error( "Resource Receive Error: {0}" )] ResourceReceiveError( #[from] ResourceReceiveError ),
}

impl From<DispatchError> for Val {
	fn from( error: DispatchError ) -> Val { match error {
		DispatchError::InvalidInterfacePath( package ) => Val::Variant( "invalid-interface-path".to_string(), Some( Box::new( Val::String( package )))),
		DispatchError::InvalidFunction( function ) => Val::Variant( "invalid-function".to_string(), Some( Box::new( Val::String( function )))),
		DispatchError::MissingResponse => Val::Variant( "missing-response".to_string(), None ),
		DispatchError::RuntimeException( exception ) => Val::Variant( "runtime-exception".to_string(), Some( Box::new( Val::String( exception.to_string() )))),
		DispatchError::InvalidArgumentList => Val::Variant( "invalid-argument-list".to_string(), None ),
		DispatchError::UnsupportedType( name ) => Val::Variant( "unsupported-type".to_string(), Some( Box::new( Val::String( name )))),
		DispatchError::ExecutorUnavailable => Val::Variant( "executor-unavailable".to_string(), None ),
		DispatchError::DispatchQueueFull => Val::Variant( "dispatch-queue-full".to_string(), None ),
		DispatchError::ResourceCreationError( err ) => err.into(),
		DispatchError::ResourceReceiveError( err ) => err.into(),
	}}
}

impl<Ctx: PluginContext + 'static> PluginInstanceSync<Ctx> {
	pub(crate) fn new_sync(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
	) -> Self {
		Self { dispatcher: Arc::new( SyncDispatcher {
			state: std::sync::Mutex::new( PluginState {
				store,
				instance,
				interface_remaps,
				fuel_limiter,
				epoch_limiter,
			}),
			admission: std::sync::Mutex::new( SyncQueue::default() ),
			changed: std::sync::Condvar::new(),
		})}
	}

	pub(crate) fn dispatch_from(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		let _permit = self.dispatcher.enter();
		lock_unpoisoned( &self.dispatcher.state )
			.dispatch( package_name, interface_name, function_name, function, data )
	}
}

impl<Ctx: 'static> SyncDispatcher<Ctx> {
	fn enter( &self ) -> SyncPermit<'_, Ctx> {
		let mut queue = lock_unpoisoned( &self.admission );
		let ticket = queue.next_ticket;
		queue.next_ticket = queue.next_ticket.wrapping_add( 1 );
		queue.waiting.push_back( ticket );
		Self::select_next( &mut queue );
		while queue.active != Some( ticket ) {
			queue = self.changed.wait( queue ).unwrap_or_else( std::sync::PoisonError::into_inner );
		}
		SyncPermit { dispatcher: self }
	}

	fn leave( &self ) {
		let mut queue = lock_unpoisoned( &self.admission );
		if queue.active.take().is_none() { return; }
		Self::select_next( &mut queue );
		self.changed.notify_all();
	}

	fn select_next( queue: &mut SyncQueue ) {
		if queue.active.is_some() { return; }
		queue.active = queue.waiting.pop_front();
	}
}

impl<Ctx: 'static> Drop for SyncPermit<'_, Ctx> {
	fn drop( &mut self ) { self.dispatcher.leave(); }
}

impl<Ctx: PluginContext + 'static> PluginInstanceAsync<Ctx> {
	pub(crate) fn new(
		store: Store<Ctx>,
		instance: Instance,
		interface_remaps: HashMap<String, Remap>,
		fuel_limiter: Option<CallLimiter<Ctx>>,
		epoch_limiter: Option<CallLimiter<Ctx>>,
		executor: impl Spawn + Send + Sync + 'static,
	) -> Self {
		let dispatcher = Arc::new( Dispatcher {
			state: Arc::new( Mutex::new( PluginState {
				store,
				instance,
				interface_remaps,
				fuel_limiter,
				epoch_limiter,
			})),
			executor: Arc::new( executor ),
			queue: std::sync::Mutex::new( DispatchQueue::default() ),
		});
		Self { dispatcher }
	}

	pub(crate) async fn dispatch_async_from(
		&self,
		caller: &Caller,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let result = self.dispatcher.enqueue(
			caller, package_name, interface_name, function_name, function, data,
		)?;
		result.await.map_err(| _ | DispatchError::ExecutorUnavailable )?
	}
}

impl<Ctx: PluginContext + 'static> Dispatcher<Ctx> {
	fn enqueue(
		self: &Arc<Self>,
		caller: &Caller,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<futures::channel::oneshot::Receiver<Result<Val, DispatchError>>, DispatchError> {
		let bytes = retained_bytes( data ).ok_or( DispatchError::DispatchQueueFull )?;
		let mut caller_budget = lock_unpoisoned( &caller.budget );
		let mut queue = lock_unpoisoned( &self.queue );
		if !has_capacity( &caller_budget, &queue, bytes ) {
			return Err( DispatchError::DispatchQueueFull );
		}

		caller_budget.calls += 1;
		caller_budget.bytes += bytes;
		queue.calls += 1;
		queue.bytes += bytes;
		drop( caller_budget );

		let ( response, result ) = futures::channel::oneshot::channel();
		let destination: Arc<dyn DestinationBudget> = self.clone();
		let pending = PendingCall {
			package_name: package_name.to_string(),
			interface_name: interface_name.to_string(),
			function_name: function_name.to_string(),
			function: function.clone(),
			data: data.to_vec(),
			response,
			_reservation: Reservation { caller: caller.clone(), destination: Arc::downgrade( &destination ), bytes },
		};
		let caller_queue = queue.callers.entry( caller.id ).or_default();
		caller_queue.calls.push_back( pending );
		if !caller_queue.ready {
			caller_queue.ready = true;
			queue.ready.push_back( caller.id );
		}
		let start_drain = !queue.draining;
		queue.draining = true;
		drop( queue );

		if start_drain {
			let dispatcher = self.clone();
			let guard = DrainGuard { dispatcher: dispatcher.clone() };
			let task: BoxFuture<'static, ()> = Box::pin( async move {
				let _guard = guard;
				dispatcher.drain().await;
			});
			if self.executor.spawn_obj( FutureObj::new( task )).is_err() {
				self.reject_all();
				return Err( DispatchError::ExecutorUnavailable );
			}
		}
		Ok( result )
	}

	async fn drain( self: Arc<Self> ) {
		loop {
			let Some(( caller_id, pending )) = self.next() else { return };
			if pending.response.is_canceled() {
				self.finish_turn( caller_id );
				continue;
			}
			let result = self.state.lock().await.dispatch_async(
				&pending.package_name,
				&pending.interface_name,
				&pending.function_name,
				&pending.function,
				&pending.data,
			).await;
			self.finish_turn( caller_id );
			let PendingCall { response, _reservation: reservation, .. } = pending;
			drop( reservation );
			let _ = response.send( result );
		}
	}

	fn next( &self ) -> Option<(u64, PendingCall)> {
		let mut queue = lock_unpoisoned( &self.queue );
		let Some( caller_id ) = queue.ready.pop_front() else {
			queue.draining = false;
			return None;
		};
		let caller_queue = queue.callers.get_mut( &caller_id )?;
		caller_queue.ready = false;
		caller_queue.calls.pop_front().map(| pending | ( caller_id, pending ))
	}

	fn finish_turn( &self, caller_id: u64 ) {
		let mut queue = lock_unpoisoned( &self.queue );
		let mut remove = false;
		let caller_queue = queue.callers.get_mut( &caller_id )
			.expect( "a caller must remain registered until its active turn finishes" );
		if caller_queue.calls.is_empty() {
			remove = true;
		} else if !caller_queue.ready {
			caller_queue.ready = true;
			queue.ready.push_back( caller_id );
		}
		if remove { queue.callers.remove( &caller_id ); }
	}

	fn reject_all( &self ) {
		let pending = {
			let mut queue = lock_unpoisoned( &self.queue );
			queue.draining = false;
			queue.ready.clear();
			queue.callers.drain().flat_map(|( _, caller )| caller.calls ).collect::<Vec<_>>()
		};
		drop( pending );
	}
}

impl<Ctx: PluginContext + 'static> Drop for DrainGuard<Ctx> {
	fn drop( &mut self ) { self.dispatcher.reject_all(); }
}

impl<Ctx: PluginContext + 'static> DestinationBudget for Dispatcher<Ctx> {
	fn release( &self, bytes: usize ) {
		let mut queue = lock_unpoisoned( &self.queue );
		queue.calls = queue.calls.saturating_sub( 1 );
		queue.bytes = queue.bytes.saturating_sub( bytes );
	}
}

impl Drop for Reservation {
	fn drop( &mut self ) {
		let mut caller = lock_unpoisoned( &self.caller.budget );
		caller.calls = caller.calls.saturating_sub( 1 );
		caller.bytes = caller.bytes.saturating_sub( self.bytes );
		drop( caller );
		if let Some( destination ) = self.destination.upgrade() { destination.release( self.bytes ); }
	}
}

fn lock_unpoisoned<T>( mutex: &std::sync::Mutex<T> ) -> std::sync::MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else( std::sync::PoisonError::into_inner )
}

pub(crate) fn clone_after_wait<T: Clone>( mutex: &Mutex<T> ) -> T {
	clone_after_wait_with( mutex, std::thread::yield_now )
}

fn clone_after_wait_with<T: Clone>( mutex: &Mutex<T>, mut wait: impl FnMut() ) -> T {
	loop {
		if let Some( value ) = mutex.try_lock() { return value.clone(); }
		wait();
	}
}

fn retained_bytes( values: &[Val] ) -> Option<usize> {
	values.iter().try_fold( 0usize, | total, value | total.checked_add( retained_value_bytes( value )? ))
}

fn has_capacity( caller: &Budget, destination: &DispatchQueue, bytes: usize ) -> bool {
	caller.calls < MAX_CALLER_CALLS
		&& caller.bytes.checked_add( bytes ).is_some_and(| total | total <= MAX_CALLER_BYTES )
		&& destination.calls < MAX_DESTINATION_CALLS
		&& destination.bytes.checked_add( bytes ).is_some_and(| total | total <= MAX_DESTINATION_BYTES )
}

fn retained_value_bytes( value: &Val ) -> Option<usize> {
	let dynamic = match value {
		Val::String( value ) | Val::Enum( value ) => value.len(),
		Val::List( values ) | Val::Tuple( values ) => retained_bytes( values )?,
		Val::Map( values ) => values.iter().try_fold( 0usize, | total, ( key, value )| {
			total.checked_add( retained_value_bytes( key )? )?.checked_add( retained_value_bytes( value )? )
		})?,
		Val::Record( values ) => values.iter().try_fold( 0usize, | total, ( name, value )| {
			total.checked_add( name.len() )?.checked_add( retained_value_bytes( value )? )
		})?,
		Val::Variant( name, value ) => name.len().checked_add( value.as_deref().map_or( Some( 0 ), retained_value_bytes )? )?,
		Val::Option( value ) | Val::Result( Ok( value )) | Val::Result( Err( value )) =>
			value.as_deref().map_or( Some( 0 ), retained_value_bytes )?,
		Val::Flags( names ) => names.iter().try_fold( 0usize, | total, name | total.checked_add( name.len() ))?,
		_ => 0,
	};
	std::mem::size_of::<Val>().checked_add( dynamic )
}

impl<Ctx: PluginContext + 'static> PluginState<Ctx> {
	const PLACEHOLDER_VAL: Val = Val::Option( None );
	const VOID_RETURN_VAL: Val = Val::Option( None );

	fn dispatch(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let mut buffer = self.prepare_call( package_name, interface_name, function_name, function )?;
		let ( exported_interface_path, exported_function_name ) = self.resolve_export( package_name, interface_name, function_name );
		let func = self.function( &exported_interface_path, &exported_function_name )?;
		let call_result = func.call( &mut self.store, data, &mut buffer );
		Self::finish_call( function, buffer, call_result )
	}

	async fn dispatch_async(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
		data: &[Val],
	) -> Result<Val, DispatchError> {
		ensure_supported_values( data )?;
		let mut buffer = self.prepare_call( package_name, interface_name, function_name, function )?;
		let ( exported_interface_path, exported_function_name ) = self.resolve_export( package_name, interface_name, function_name );
		let func = self.function( &exported_interface_path, &exported_function_name )?;
		let call_result = func.call_async( &mut self.store, data, &mut buffer ).await;
		Self::finish_call( function, buffer, call_result )
	}

	fn prepare_call(
		&mut self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
		function: &Function,
	) -> Result<Vec<Val>, DispatchError> {
		let canonical_interface_path = format!( "{}/{}", package_name, interface_name );
		if let Some( mut limiter ) = self.fuel_limiter.take() {
			let fuel = limiter( &mut self.store, &canonical_interface_path, function_name, function );
			self.fuel_limiter = Some( limiter );
			self.store.set_fuel( fuel ).map_err( DispatchError::RuntimeException )?;
		}
		if let Some( mut limiter ) = self.epoch_limiter.take() {
			let ticks = limiter( &mut self.store, &canonical_interface_path, function_name, function );
			self.epoch_limiter = Some( limiter );
			self.store.set_epoch_deadline( ticks );
		}
		Ok( match function.return_kind() != ReturnKind::Void {
			true => vec![ Self::PLACEHOLDER_VAL ],
			false => Vec::with_capacity( 0 ),
		})
	}

	fn function( &mut self, interface_path: &str, function_name: &str ) -> Result<wasmtime::component::Func, DispatchError> {
		let interface_index = self.instance
			.get_export_index( &mut self.store, None, interface_path )
			.ok_or_else(|| DispatchError::InvalidInterfacePath( interface_path.to_string() ))?;
		let func_index = self.instance
			.get_export_index( &mut self.store, Some( &interface_index ), function_name )
			.ok_or_else(|| DispatchError::InvalidFunction( format!( "{interface_path}:{function_name}" )))?;
		self.instance
			.get_func( &mut self.store, func_index )
			.ok_or_else(|| DispatchError::InvalidFunction( format!( "{interface_path}:{function_name}" )))
	}

	fn finish_call(
		function: &Function,
		mut buffer: Vec<Val>,
		call_result: Result<(), wasmtime::Error>,
	) -> Result<Val, DispatchError> {
		call_result.map_err( DispatchError::RuntimeException )?;
		let result = match function.return_kind() != ReturnKind::Void {
			true => buffer.pop().ok_or( DispatchError::MissingResponse )?,
			false => Self::VOID_RETURN_VAL,
		};
		ensure_supported_value( &result )?;
		Ok( result )
	}

	fn resolve_export( &self, package_name: &str, interface_name: &str, function_name: &str ) -> (String, String) {
		match self.interface_remaps.get( interface_name ) {
			Some( remap ) => (
				format!( "{}/{}", package_name, remap.interface_name( interface_name )),
				remap.item_name( function_name ).to_string(),
			),
			None => (
				format!( "{}/{}", package_name, interface_name ),
				function_name.to_string(),
			),
		}
	}

}

fn ensure_supported_values( values: &[Val] ) -> Result<(), DispatchError> {
	values.iter().try_for_each( ensure_supported_value )
}

fn ensure_supported_value( value: &Val ) -> Result<(), DispatchError> {
	match value {
		Val::List( values ) | Val::Tuple( values ) => ensure_supported_values( values ),
		Val::Map( values ) => values.iter().try_for_each(|( key, value )| {
			ensure_supported_value( key )?;
			ensure_supported_value( value )
		}),
		Val::Record( values ) => values.iter().try_for_each(|( _, value )| ensure_supported_value( value )),
		Val::Variant( _, Some( value ))
		| Val::Option( Some( value ))
		| Val::Result( Ok( Some( value )))
		| Val::Result( Err( Some( value ))) => ensure_supported_value( value ),
		Val::Future( _ ) => Err( DispatchError::UnsupportedType( "future".to_string() )),
		Val::Stream( _ ) => Err( DispatchError::UnsupportedType( "stream".to_string() )),
		Val::ErrorContext( _ ) => Err( DispatchError::UnsupportedType( "error-context".to_string() )),
		_ => Ok(()),
	}
}

#[cfg(test)] mod tests { include!( "plugin_instance_tests.rs" ); }
