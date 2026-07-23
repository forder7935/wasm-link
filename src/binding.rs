//! Binding specification and metadata types.
//!
//! A [`Binding`] defines an abstract contract specifying what plugins must implement
//! (via plugs) or what they could depend on (via sockets). It bundles one or more WIT
//! [`Interface`]s under a single identifier.

use std::sync::Arc ;
use std::collections::HashMap ;
use nonempty_collections::NonEmptyIterator ;
use wasmtime::component::{ Linker, Val };

use crate::{ Interface, PluginContext };
use crate::cardinality::{ Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne };
use crate::plugin_instance::{
	AsyncDispatchInstance, Caller, ExportEffectInstance, PluginInstanceAsync, PluginInstanceSync,
};



type PluginSockets<PluginId, Plugins, Instance> =
	<Plugins as Cardinality<PluginId, Instance>>::Rebind<Arc<Instance>> ;

type DispatchResults<PluginId, Plugins, Instance> =
	<PluginSockets<PluginId, Plugins, Instance> as Cardinality<PluginId, Arc<Instance>>>::Rebind<
		Result<wasmtime::component::Val, crate::DispatchError>
	>;

type DispatchVals<PluginId, Plugins, Instance> =
	<PluginSockets<PluginId, Plugins, Instance> as Cardinality<PluginId, Arc<Instance>>>::Rebind<
		wasmtime::component::Val
	>;

pub(crate) trait ExportEffects {
	fn has_async_export(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool>;
}

fn any_async_export( mut effects: impl Iterator<Item = bool> ) -> Option<bool> {
	let effect = effects.next()?;
	Some( effect || effects.any( std::convert::identity ))
}

impl<Id, Instance: ExportEffectInstance> ExportEffects for ExactlyOne<Id, Arc<Instance>> {
	fn has_async_export(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool> {
		Some( self.1.export_is_async( package_name, interface_name, function_name ))
	}
}

impl<Id, Instance: ExportEffectInstance> ExportEffects for AtMostOne<Id, Arc<Instance>> {
	fn has_async_export(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool> {
		self.0.as_ref().map(|( _, instance )|
			instance.export_is_async( package_name, interface_name, function_name )
		)
	}
}

impl<Id, Instance> ExportEffects for AtLeastOne<Id, Arc<Instance>>
where
	Id: std::hash::Hash + Eq,
	Instance: ExportEffectInstance,
{
	fn has_async_export(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool> {
		any_async_export(
			self.0.nonempty_iter().map(|( _, instance )|
				instance.export_is_async( package_name, interface_name, function_name )
			).collect::<Vec<_>>().into_iter()
		)
	}
}

impl<Id, Instance> ExportEffects for Any<Id, Arc<Instance>>
where
	Id: std::hash::Hash + Eq,
	Instance: ExportEffectInstance,
{
	fn has_async_export(
		&self,
		package_name: &str,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool> {
		any_async_export( self.0.values().map(| instance |
			instance.export_is_async( package_name, interface_name, function_name )
		))
	}
}

struct BindingData<PluginId, Plugins, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	PluginSockets<PluginId, Plugins, Instance>: Send + Sync,
{
	package_name: String,
	interfaces: HashMap<String, Interface>,
	plugins: PluginSockets<PluginId, Plugins, Instance>,
	caller: Caller,
}

/// An abstract contract specifying what plugins must implement (via plugs) or what
/// they could depend on (via sockets). It bundles one or more WIT [`Interface`]s
/// under a single package name.
///
/// `Binding` is a handle to shared state. Cloning a `Binding` creates another handle
/// to the same underlying binding, enabling shared dependencies where multiple
/// plugins depend on the same binding.
///
/// ```
/// # use std::collections::{ HashMap, HashSet };
/// # use wasm_link::{ Binding, Interface, Function, FunctionKind, ReturnKind, Plugin, Engine, Component, Linker, ResourceTable };
/// # use wasm_link::cardinality::ExactlyOne ;
/// # struct Ctx { resource_table: ResourceTable }
/// # impl wasm_link::PluginContext for Ctx {
/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
/// # }
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let engine = Engine::default();
/// # let linker = Linker::new( &engine );
/// # let plugin = Plugin::new( Component::new( &engine, "(component)" )?, Ctx { resource_table: ResourceTable::new() }).instantiate( &engine, &linker )?;
/// let binding: Binding<String, Ctx> = Binding::new(
/// 	"my:package",
/// 	HashMap::from([
/// 		( "api".to_string(), Interface::new(
/// 			HashMap::from([( "get-value".into(), Function::new(
/// 				FunctionKind::Freestanding,
/// 				ReturnKind::MayContainResources,
/// 			))]),
/// 			HashSet::from([ "my-resource".to_string() ]),
/// 		)),
/// 	]),
/// 	ExactlyOne( "my-plugin".to_string(), plugin ),
/// );
///
/// // Clone for shared dependencies - both refer to the same binding
/// let binding_clone = binding.clone();
/// # let binding_any_clone = binding.into_any().clone();
/// # Ok(())
/// # }
/// ```
///
/// # Type Parameters
/// - `PluginId`: Unique identifier type for plugins (e.g., `String`, `UUID`)
/// - `Ctx`: Context stored in each plugin's Wasmtime store
/// - `Plugins`: Cardinality wrapper containing the plugin instances
/// - `Instance`: [`PluginInstanceSync`] or [`PluginInstanceAsync`]
pub struct Binding<PluginId, Ctx, Plugins = ExactlyOne<PluginId, PluginInstanceSync<Ctx>>, Instance = PluginInstanceSync<Ctx>>(
	Arc<BindingData<PluginId, Plugins, Instance>>,
	std::marker::PhantomData<fn() -> Ctx>,
)
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance> + 'static,
	PluginSockets<PluginId, Plugins, Instance>: Send + Sync;

impl<PluginId, Ctx, Plugins, Instance> Clone for Binding<PluginId, Ctx, Plugins, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance> + 'static,
	PluginSockets<PluginId, Plugins, Instance>: Send + Sync,
{
	/// Creates another handle to the same underlying binding, enabling shared dependencies where
	/// multiple plugins depend on the same binding.
	fn clone( &self ) -> Self {
		Self( Arc::clone( &self.0 ), std::marker::PhantomData )
	}
}

impl<PluginId, Ctx, Plugins, Instance> std::fmt::Debug for Binding<PluginId, Ctx, Plugins, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
	Ctx: PluginContext + std::fmt::Debug + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance> + 'static,
	PluginSockets<PluginId, Plugins, Instance>: Send + Sync + std::fmt::Debug,
{
	fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		f.debug_struct( "Binding" )
			.field( "package_name", &self.0.package_name )
			.field( "interfaces", &self.0.interfaces )
			.field( "plugins", &self.0.plugins )
			.finish()
	}
}

impl<PluginId, Ctx, Plugins, Instance> Binding<PluginId, Ctx, Plugins, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance> + 'static,
	PluginSockets<PluginId, Plugins, Instance>: Cardinality<PluginId, Arc<Instance>> + Send + Sync,
{

	/// Creates a new binding specification.
	pub fn new(
		package_name: impl Into<String>,
		interfaces: HashMap<String, Interface>,
		plugins: Plugins
	) -> Self {
		Self( Arc::new( BindingData {
			package_name: package_name.into(),
			interfaces,
			plugins: plugins.map_mut( Arc::new ),
			caller: Caller::new(),
		}), std::marker::PhantomData )
	}

	pub(crate) fn plugins( &self ) -> &PluginSockets<PluginId, Plugins, Instance> {
		&self.0.plugins
	}

	pub(crate) fn has_async_export(
		&self,
		interface_name: &str,
		function_name: &str,
	) -> Option<bool>
	where
		PluginSockets<PluginId, Plugins, Instance>: ExportEffects,
	{
		self.0.plugins.has_async_export(
			&self.0.package_name,
			interface_name,
			function_name,
		)
	}
}

impl<PluginId, Ctx, Plugins> Binding<PluginId, Ctx, Plugins, PluginInstanceSync<Ctx>>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<PluginId, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<PluginId, Plugins, PluginInstanceSync<Ctx>>: Cardinality<PluginId, Arc<PluginInstanceSync<Ctx>>> + Send + Sync,
{

	pub(crate) fn add_to_linker(
		binding: &Binding<PluginId, Ctx, Plugins>,
		linker: &mut Linker<Ctx>,
		caller: &Caller,
	) -> Result<(), wasmtime::Error>
	where
		PluginId: Into<Val>,
		DispatchVals<PluginId, Plugins, PluginInstanceSync<Ctx>>: Into<Val>,
	{
		binding.0.interfaces.iter().try_for_each(|( name, interface )| {
			let interface_ident = format!( "{}/{}", binding.0.package_name, name );
			interface.add_to_linker( linker, &binding.0.package_name, &interface_ident, name, binding, caller )
		})
	}

	/// Dispatches a function call to all plugins implementing this binding.
	///
	/// This is used for external dispatch (calling into the plugin graph from outside).
	/// The result is wrapped in a type matching the binding's cardinality.
	///
	/// # Arguments
	/// * `interface_name` - The interface name within this binding (e.g., "example")
	/// * `function_name` - The function name within the interface (e.g., "get-value")
	/// * `args` - Arguments to pass to the function
	///
	/// # Returns
	/// A cardinality wrapper containing `Result<Val, DispatchError>` for each plugin.
	/// For [`ReturnKind::Void`]( crate::ReturnKind::Void ), the value is an empty tuple
	/// (`Val::Option( None )`) placeholder.
	///
	/// # Errors
	/// Returns an error if the interface or function is not found in this binding.
	/// Concurrent calls wait in caller-aware round-robin order when the destination
	/// is already executing.
	pub fn dispatch(
		&self,
		interface_name: &str,
		function_name: &str,
		args: &[wasmtime::component::Val],
	) -> Result<DispatchResults<PluginId, Plugins, PluginInstanceSync<Ctx>>, crate::DispatchError> {

		let interface = self.0.interfaces.get( interface_name )
			.ok_or_else(|| crate::DispatchError::InvalidInterfacePath( format!( "{}/{}", self.0.package_name, interface_name )))?;

		let function = interface.function( function_name )
			.ok_or_else(|| crate::DispatchError::InvalidFunction( function_name.to_string() ))?;

		Ok( self.0.plugins.map(| _, plugin | {
			plugin.dispatch_from(
				&self.0.caller,
				&self.0.package_name,
				interface_name,
				function_name,
				function,
				args,
			)
		}))

	}


}

impl<PluginId, Ctx, Plugins, Executor> Binding<PluginId, Ctx, Plugins, PluginInstanceAsync<Ctx, Executor>>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, PluginInstanceAsync<Ctx, Executor>> + 'static,
	PluginSockets<PluginId, Plugins, PluginInstanceAsync<Ctx, Executor>>:
		Cardinality<PluginId, Arc<PluginInstanceAsync<Ctx, Executor>>> + Send + Sync,
{
	/// Asynchronously dispatches a function call to all plugins implementing this binding.
	///
	/// This method queues behind a busy plugin.
	/// Queued calls are serviced round-robin by caller. A call is rejected with
	/// [`DispatchError::DispatchQueueFull`](crate::DispatchError::DispatchQueueFull)
	/// when its caller or destination reaches an async dispatch count or byte bound.
	/// It is required for instances created by [`Plugin::instantiate_async`](crate::Plugin::instantiate_async)
	/// or [`Plugin::link_async`](crate::Plugin::link_async).
	///
	/// # Example
	///
	/// ```
	/// # use std::collections::{ HashMap, HashSet };
	/// # use wasm_link::{ Binding, Component, Engine, Function, FunctionKind, Interface, Linker, Plugin, PluginContext, ResourceTable, ReturnKind, Val };
	/// # use wasm_link::cardinality::ExactlyOne;
	/// # struct Context { table: ResourceTable }
	/// # impl PluginContext for Context { fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table } }
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> { futures::executor::block_on( async {
	/// # let engine = Engine::default();
	/// # let linker = Linker::new( &engine );
	/// # let executor = futures::executor::ThreadPool::new()?;
	/// # let component = Component::new( &engine, r#"(component
	/// # 	(core module $m (func (export "get") (result i32) i32.const 42))
	/// # 	(core instance $i (instantiate $m))
	/// # 	(func $get (result u32) (canon lift (core func $i "get")))
	/// # 	(instance $root (export "get" (func $get)))
	/// # 	(export "example:plugin/root" (instance $root))
	/// # )"# )?;
	/// # let plugin = Plugin::new( component, Context { table: ResourceTable::new() })
	/// # 	.instantiate_async( &engine, &linker, executor ).await?;
	/// # let binding = Binding::new(
	/// # 	"example:plugin",
	/// # 	HashMap::from([( "root".to_string(), Interface::new(
	/// # 		HashMap::from([( "get".to_string(), Function::new( FunctionKind::Freestanding, ReturnKind::MayContainResources ))]),
	/// # 		HashSet::new(),
	/// # 	))]),
	/// # 	ExactlyOne( "plugin".to_string(), plugin ),
	/// # );
	/// let result = binding.dispatch( "root", "get", &[] ).await?;
	/// assert!( matches!( result, ExactlyOne( _, Ok( Val::U32( 42 )))));
	/// # Ok(()) }) }
	/// ```
	///
	/// # Errors
	/// Returns an error if the interface or function is not found in this binding.
	pub async fn dispatch(
		&self,
		interface_name: &str,
		function_name: &str,
		args: &[wasmtime::component::Val],
	) -> Result<DispatchResults<PluginId, Plugins, PluginInstanceAsync<Ctx, Executor>>, crate::DispatchError>
	where
		PluginId: Into<Val>,
		DispatchResults<PluginId, Plugins, PluginInstanceAsync<Ctx, Executor>>: Send,
	{
		let interface = self.0.interfaces.get( interface_name )
			.ok_or_else(|| crate::DispatchError::InvalidInterfacePath( format!( "{}/{}", self.0.package_name, interface_name )))?;
		let function = interface.function( function_name )
			.ok_or_else(|| crate::DispatchError::InvalidFunction( function_name.to_string() ))?;
		let package_name = self.0.package_name.as_str();
		let caller = &self.0.caller;

		Ok( self.0.plugins.map_async(| _, plugin | {
			async move {
				plugin.dispatch_async_from(
					caller,
					package_name,
					interface_name,
					function_name,
					function,
					args,
				).await
			}
		}).await )
	}

}

fn add_to_linker_async<PluginId, Ctx, Plugins, Instance, Executor>(
	binding: &Binding<PluginId, Ctx, Plugins, Instance>,
	linker: &mut Linker<Ctx>,
	caller: &Caller,
	executor: &Arc<Executor>,
) -> Result<(), wasmtime::Error>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
	Instance: AsyncDispatchInstance<Ctx, Executor>,
	Plugins: Cardinality<PluginId, Instance> + 'static,
	PluginSockets<PluginId, Plugins, Instance>: Cardinality<PluginId, Arc<Instance>> + Send + Sync,
	PluginSockets<PluginId, Plugins, Instance>: ExportEffects,
	PluginId: Into<Val>,
	DispatchVals<PluginId, Plugins, Instance>: Into<Val> + Send,
{
	binding.0.interfaces.iter().try_for_each(|( name, interface )| {
		let interface_ident = format!( "{}/{}", binding.0.package_name, name );
		interface.add_to_linker_async(
			linker, &binding.0.package_name, &interface_ident, name, binding,
			caller, executor,
		)
	})
}

/// Type-erased binding wrapper for heterogeneous socket lists.
///
/// Use when a plugin's sockets include bindings with different cardinalities.
#[derive( Debug )]
pub enum BindingAny<PluginId, Ctx, Instance = PluginInstanceSync<Ctx>>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	/// Exactly one plugin implementation.
	ExactlyOne( Binding<PluginId, Ctx, ExactlyOne<PluginId, Instance>, Instance> ),
	/// Zero or one plugin implementation.
	AtMostOne( Binding<PluginId, Ctx, AtMostOne<PluginId, Instance>, Instance> ),
	/// One or more plugin implementations.
	AtLeastOne( Binding<PluginId, Ctx, AtLeastOne<PluginId, Instance>, Instance> ),
	/// Zero or more plugin implementations.
	Any( Binding<PluginId, Ctx, Any<PluginId, Instance>, Instance> ),
}

impl<PluginId, Ctx> BindingAny<PluginId, Ctx, PluginInstanceSync<Ctx>>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext + 'static,
{
	pub(crate) fn add_to_linker( &self, linker: &mut Linker<Ctx>, caller: &Caller ) -> Result<(), wasmtime::Error> {
		match self {
			Self::ExactlyOne( binding ) => Binding::add_to_linker( binding, linker, caller ),
			Self::AtMostOne( binding ) => Binding::add_to_linker( binding, linker, caller ),
			Self::AtLeastOne( binding ) => Binding::add_to_linker( binding, linker, caller ),
			Self::Any( binding ) => Binding::add_to_linker( binding, linker, caller ),
		}
	}

	pub(crate) fn add_to_linker_async<Executor>(
		&self,
		linker: &mut Linker<Ctx>,
		caller: &Caller,
		executor: &Arc<Executor>,
	) -> Result<(), wasmtime::Error>
	where
		Executor: futures::task::Spawn + Send + Sync + 'static,
	{
		match self {
			Self::ExactlyOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::AtMostOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::AtLeastOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::Any( binding ) => add_to_linker_async( binding, linker, caller, executor ),
		}
	}

}

impl<PluginId, Ctx, Executor> BindingAny<PluginId, Ctx, PluginInstanceAsync<Ctx, Executor>>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	pub(crate) fn add_to_linker_async(
		&self,
		linker: &mut Linker<Ctx>,
		caller: &Caller,
		executor: &Arc<Executor>,
	) -> Result<(), wasmtime::Error> {
		match self {
			Self::ExactlyOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::AtMostOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::AtLeastOne( binding ) => add_to_linker_async( binding, linker, caller, executor ),
			Self::Any( binding ) => add_to_linker_async( binding, linker, caller, executor ),
		}
	}
}

/// A type-erased socket accepted by [`Plugin::link_async`](crate::Plugin::link_async).
///
/// This wrapper preserves whether the destination was instantiated synchronously or
/// asynchronously. Homogeneous socket iterators are converted automatically; only
/// explicitly heterogeneous collections need `.into()` at their insertion sites.
#[derive( Debug )]
pub enum SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	/// A synchronously instantiated destination.
	Sync( BindingAny<PluginId, Ctx, PluginInstanceSync<Ctx>> ),
	/// An asynchronously instantiated destination.
	Async( BindingAny<PluginId, Ctx, PluginInstanceAsync<Ctx, Executor>> ),
}

impl<PluginId, Ctx, Executor> Clone for SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	fn clone( &self ) -> Self {
		match self {
			Self::Sync( binding ) => Self::Sync( binding.clone() ),
			Self::Async( binding ) => Self::Async( binding.clone() ),
		}
	}
}

impl<PluginId, Ctx, Executor> SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	pub(crate) fn add_to_linker(
		&self,
		linker: &mut Linker<Ctx>,
		caller: &Caller,
		executor: &Arc<Executor>,
	) -> Result<(), wasmtime::Error> {
		match self {
			Self::Sync( binding ) => binding.add_to_linker_async( linker, caller, executor ),
			Self::Async( binding ) => binding.add_to_linker_async( linker, caller, executor ),
		}
	}
}

impl<PluginId, Ctx, Executor> From<BindingAny<PluginId, Ctx, PluginInstanceSync<Ctx>>>
	for SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	fn from( binding: BindingAny<PluginId, Ctx, PluginInstanceSync<Ctx>> ) -> Self { Self::Sync( binding ) }
}

impl<PluginId, Ctx, Executor> From<BindingAny<PluginId, Ctx, PluginInstanceAsync<Ctx, Executor>>>
	for SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
{
	fn from( binding: BindingAny<PluginId, Ctx, PluginInstanceAsync<Ctx, Executor>> ) -> Self { Self::Async( binding ) }
}

impl<PluginId, Ctx, Plugins, Executor> From<Binding<PluginId, Ctx, Plugins, PluginInstanceSync<Ctx>>>
	for SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<PluginId, Plugins, PluginInstanceSync<Ctx>>: Send + Sync,
	BindingAny<PluginId, Ctx, PluginInstanceSync<Ctx>>: From<Binding<PluginId, Ctx, Plugins, PluginInstanceSync<Ctx>>>,
{
	fn from( binding: Binding<PluginId, Ctx, Plugins, PluginInstanceSync<Ctx>> ) -> Self {
		Self::Sync( binding.into() )
	}
}

impl<PluginId, Ctx, Plugins, Executor>
	From<Binding<PluginId, Ctx, Plugins, PluginInstanceAsync<Ctx, Executor>>>
	for SocketBindingAny<PluginId, Ctx, Executor>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Executor: futures::task::Spawn + Send + Sync + 'static,
	Plugins: Cardinality<PluginId, PluginInstanceAsync<Ctx, Executor>> + 'static,
	PluginSockets<PluginId, Plugins, PluginInstanceAsync<Ctx, Executor>>: Send + Sync,
	BindingAny<PluginId, Ctx, PluginInstanceAsync<Ctx, Executor>>:
		From<Binding<PluginId, Ctx, Plugins, PluginInstanceAsync<Ctx, Executor>>>,
{
	fn from( binding: Binding<PluginId, Ctx, Plugins, PluginInstanceAsync<Ctx, Executor>> ) -> Self {
		Self::Async( binding.into() )
	}
}

impl<PluginId, Ctx, Instance> From<Binding<PluginId, Ctx, ExactlyOne<PluginId, Instance>, Instance>> for BindingAny<PluginId, Ctx, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	fn from( binding: Binding<PluginId, Ctx, ExactlyOne<PluginId, Instance>, Instance> ) -> Self {
		Self::ExactlyOne( binding )
	}
}

impl<PluginId, Ctx, Instance> From<Binding<PluginId, Ctx, AtMostOne<PluginId, Instance>, Instance>> for BindingAny<PluginId, Ctx, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	fn from( binding: Binding<PluginId, Ctx, AtMostOne<PluginId, Instance>, Instance> ) -> Self {
		Self::AtMostOne( binding )
	}
}

impl<PluginId, Ctx, Instance> From<Binding<PluginId, Ctx, AtLeastOne<PluginId, Instance>, Instance>> for BindingAny<PluginId, Ctx, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	fn from( binding: Binding<PluginId, Ctx, AtLeastOne<PluginId, Instance>, Instance> ) -> Self {
		Self::AtLeastOne( binding )
	}
}

impl<PluginId, Ctx, Instance> From<Binding<PluginId, Ctx, Any<PluginId, Instance>, Instance>> for BindingAny<PluginId, Ctx, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	fn from( binding: Binding<PluginId, Ctx, Any<PluginId, Instance>, Instance> ) -> Self {
		Self::Any( binding )
	}
}

impl<PluginId, Ctx, Plugins, Instance> Binding<PluginId, Ctx, Plugins, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
	Plugins: Cardinality<PluginId, Instance>,
	PluginSockets<PluginId, Plugins, Instance>: Send + Sync,
	BindingAny<PluginId, Ctx, Instance>: From<Binding<PluginId, Ctx, Plugins, Instance>>,
{
	/// Converts this binding into a type-erased [`BindingAny`] for heterogeneous socket lists.
	pub fn into_any( self ) -> BindingAny<PluginId, Ctx, Instance> {
		self.into()
	}
}

impl<PluginId, Ctx, Instance> Clone for BindingAny<PluginId, Ctx, Instance>
where
	PluginId: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Instance: Send + Sync + 'static,
{
	/// Creates another handle to the same underlying binding, enabling shared dependencies where
	/// multiple plugins depend on the same binding.
	fn clone( &self ) -> Self {
		match self {
			Self::ExactlyOne( binding ) => Self::ExactlyOne( binding.clone() ),
			Self::AtMostOne( binding ) => Self::AtMostOne( binding.clone() ),
			Self::AtLeastOne( binding ) => Self::AtLeastOne( binding.clone() ),
			Self::Any( binding ) => Self::Any( binding.clone() ),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Effect( bool );

	impl ExportEffectInstance for Effect {
		fn export_is_async( &self, _: &str, _: &str, _: &str ) -> bool { self.0 }
	}

	#[test]
	fn aggregates_export_effects_across_optional_and_collection_cardinalities() {
		let optional = AtMostOne( Some(( "sync", Arc::new( Effect( false )))));
		assert_eq!( optional.has_async_export( "", "", "" ), Some( false ));

		let collection = Any( HashMap::from([
			( "sync", Arc::new( Effect( false ))),
			( "async", Arc::new( Effect( true ))),
		]));
		assert_eq!( collection.has_async_export( "", "", "" ), Some( true ));
	}
}
