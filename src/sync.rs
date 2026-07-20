//! Synchronous plugin runtime.
//!
//! Every type that carries runtime state is distinct from its counterpart in
//! [`crate::concurrent`], so the two runtimes cannot be mixed in one plugin tree.
//!
//! ```compile_fail
//! use wasm_link::{ FunctionKind, ReturnKind };
//! let _ = wasm_link::sync::Function::new_async(
//! 	FunctionKind::Freestanding,
//! 	ReturnKind::Void,
//! );
//! ```
//!
//! ```compile_fail
//! # use wasm_link::{ PluginContext, ResourceTable };
//! # struct Context( ResourceTable );
//! # impl PluginContext for Context {
//! # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.0 }
//! # }
//! fn takes_sync( _plugin: wasm_link::sync::PluginInstance<Context> ) {}
//! fn cannot_mix( plugin: wasm_link::concurrent::PluginInstance<Context> ) {
//! 	takes_sync( plugin );
//! }
//! ```

use std::collections::{ HashMap, HashSet };
use std::sync::Arc;

use futures::lock::Mutex;
use wasmtime::component::{ Component, Linker, Val };
use wasmtime::{ Engine, Store };

use crate::binding::{ Binding as BindingCore, BindingAny as BindingAnyCore };
use crate::cardinality::{ Any, AtLeastOne, AtMostOne, Cardinality, ExactlyOne };
use crate::interface::{ Function as FunctionMetadata, Interface as InterfaceMetadata };
use crate::plugin::Plugin as PluginCore;
use crate::plugin_instance::PluginInstanceSync;
use crate::{ DispatchError, FunctionKind, PluginContext, Remap, ReturnKind };

/// An instantiated plugin in the synchronous runtime.
pub use crate::plugin_instance::PluginInstanceSync as PluginInstance;

/// Metadata for a synchronous WIT function.
///
/// This type has no state capable of representing an asynchronous function.
#[derive( Debug, Clone )]
pub struct Function {
	kind: FunctionKind,
	return_kind: ReturnKind,
}

impl Function {
	/// Creates function metadata.
	pub fn new( kind: FunctionKind, return_kind: ReturnKind ) -> Self {
		Self { kind, return_kind }
	}

	/// Returns whether the function is freestanding or a resource method.
	pub fn kind( &self ) -> FunctionKind { self.kind }

	/// Returns how dispatch handles the function's return value.
	pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

	fn into_metadata( self ) -> FunctionMetadata {
		FunctionMetadata::new( self.kind, self.return_kind )
	}

	fn from_metadata( metadata: &FunctionMetadata ) -> Self {
		Self::new( metadata.kind(), metadata.return_kind() )
	}
}

/// A synchronous WIT interface declaration.
#[derive( Debug, Clone, Default )]
pub struct Interface {
	functions: HashMap<String, Function>,
	resources: HashSet<String>,
}

impl Interface {
	/// Creates an interface containing only synchronous functions.
	pub fn new( functions: HashMap<String, Function>, resources: HashSet<String> ) -> Self {
		Self { functions, resources }
	}

	fn into_metadata( self ) -> InterfaceMetadata {
		InterfaceMetadata::new(
			self.functions.into_iter()
				.map(|( name, function )| ( name, function.into_metadata() ))
				.collect(),
			self.resources,
		)
	}
}

type PluginSockets<Id, Ctx, Plugins> =
	<Plugins as Cardinality<Id, PluginInstanceSync<Ctx>>>::Rebind<Arc<Mutex<PluginInstanceSync<Ctx>>>>;
type Results<Id, Ctx, Plugins> =
	<PluginSockets<Id, Ctx, Plugins> as Cardinality<Id, Arc<Mutex<PluginInstanceSync<Ctx>>>>>::Rebind<Result<Val, DispatchError>>;

/// A binding in the synchronous runtime.
pub struct Binding<Id, Ctx, Plugins = ExactlyOne<Id, PluginInstanceSync<Ctx>>>(
	BindingCore<Id, Ctx, Plugins, PluginInstanceSync<Ctx>>,
)
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Send + Sync;

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Cardinality<Id, Arc<Mutex<PluginInstanceSync<Ctx>>>> + Send + Sync,
{
	/// Creates a binding.
	pub fn new(
		package_name: impl Into<String>,
		interfaces: HashMap<String, Interface>,
		plugins: Plugins,
	) -> Self {
		Self( BindingCore::new(
			package_name,
			interfaces.into_iter()
				.map(|( name, interface )| ( name, interface.into_metadata() ))
				.collect(),
			plugins,
		))
	}

	/// Dispatches a function call.
	///
	/// # Errors
	///
	/// Returns an error if the interface or function is not declared by the binding.
	pub fn dispatch(
		&self,
		interface_name: &str,
		function_name: &str,
		args: &[Val],
	) -> Result<Results<Id, Ctx, Plugins>, DispatchError> {
		self.0.dispatch( interface_name, function_name, args )
	}
}

impl<Id, Ctx, Plugins> Clone for Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Send + Sync,
{
	fn clone( &self ) -> Self { Self( self.0.clone() ) }
}

impl<Id, Ctx, Plugins> std::fmt::Debug for Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Debug + 'static,
	Ctx: PluginContext + std::fmt::Debug + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Send + Sync + std::fmt::Debug,
{
	fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		self.0.fmt( formatter )
	}
}

/// A synchronous binding with erased cardinality.
#[derive( Debug )]
pub struct BindingAny<Id, Ctx>( BindingAnyCore<Id, Ctx, PluginInstanceSync<Ctx>> )
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static;

impl<Id, Ctx> Clone for BindingAny<Id, Ctx>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
{
	fn clone( &self ) -> Self { Self( self.0.clone() ) }
}

impl<Id, Ctx> BindingAny<Id, Ctx>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
{
	fn into_core( self ) -> BindingAnyCore<Id, Ctx, PluginInstanceSync<Ctx>> {
		self.0
	}
}

macro_rules! binding_from {
	( $cardinality:ident ) => {
		impl<Id, Ctx> From<Binding<Id, Ctx, $cardinality<Id, PluginInstanceSync<Ctx>>>> for BindingAny<Id, Ctx>
		where
			Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
			Ctx: PluginContext + 'static,
		{
			fn from( binding: Binding<Id, Ctx, $cardinality<Id, PluginInstanceSync<Ctx>>> ) -> Self {
				Self( binding.0.into() )
			}
		}
	};
}

binding_from!( ExactlyOne );
binding_from!( AtMostOne );
binding_from!( AtLeastOne );
binding_from!( Any );

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Send + Sync,
	BindingAny<Id, Ctx>: From<Self>,
{
	/// Erases this binding's cardinality.
	pub fn into_any( self ) -> BindingAny<Id, Ctx> { self.into() }
}

/// A component and context configured for the synchronous runtime.
#[must_use = "call .instantiate() or .link() to create a PluginInstance"]
pub struct Plugin<Ctx: 'static>( PluginCore<Ctx> );

impl<Ctx> Plugin<Ctx>
where
	Ctx: PluginContext + 'static,
{
	/// Creates a plugin declaration.
	pub fn new( component: Component, context: Ctx ) -> Self {
		Self( PluginCore::new( component, context ))
	}

	/// Sets the fuel available during instantiation.
	pub fn with_initial_fuel( mut self, fuel: u64 ) -> Self {
		self.0 = self.0.with_initial_fuel( fuel );
		self
	}

	/// Sets the per-call fuel limiter.
	pub fn with_fuel_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_fuel_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &Function::from_metadata( function ))
		});
		self
	}

	/// Sets the per-call epoch deadline limiter.
	pub fn with_epoch_limiter(
		mut self,
		mut limiter: impl FnMut( &mut Store<Ctx>, &str, &str, &Function ) -> u64 + Send + 'static,
	) -> Self {
		self.0 = self.0.with_epoch_limiter( move | store, interface, name, function | {
			limiter( store, interface, name, &Function::from_metadata( function ))
		});
		self
	}

	/// Installs a Wasmtime memory/table limiter from the context.
	pub fn with_memory_limiter(
		mut self,
		limiter: impl (FnMut( &mut Ctx ) -> &mut dyn wasmtime::ResourceLimiter) + Send + Sync + 'static,
	) -> Self {
		self.0 = self.0.with_memory_limiter( limiter );
		self
	}

	/// Remaps requested interfaces to component exports.
	pub fn remap_interfaces( mut self, remaps: HashMap<String, Remap> ) -> Self {
		self.0 = self.0.remap_interfaces( remaps );
		self
	}

	/// Links socket bindings and instantiates the plugin.
	///
	/// # Errors
	///
	/// Returns an error when linking, validation, or instantiation fails.
	pub fn link<Id, Sockets>(
		self,
		engine: &Engine,
		linker: Linker<Ctx>,
		sockets: Sockets,
	) -> Result<PluginInstance<Ctx>, wasmtime::Error>
	where
		Id: Eq + std::hash::Hash + Clone + std::fmt::Debug + Send + Sync + Into<Val> + 'static,
		Sockets: IntoIterator,
		Sockets::Item: Into<BindingAny<Id, Ctx>>,
	{
		self.0.ensure_synchronous( engine )?;
		self.0.link(
			engine,
			linker,
			sockets.into_iter().map(| binding | binding.into().into_core()),
		)
	}

	/// Instantiates a plugin with no socket bindings.
	///
	/// # Errors
	///
	/// Returns an error when synchronous instantiation fails.
	pub fn instantiate(
		self,
		engine: &Engine,
		linker: &Linker<Ctx>,
	) -> Result<PluginInstance<Ctx>, wasmtime::Error> {
		self.0.ensure_synchronous( engine )?;
		self.0.instantiate( engine, linker )
	}
}

impl<Ctx: std::fmt::Debug + 'static> std::fmt::Debug for Plugin<Ctx> {
	fn fmt( &self, formatter: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
		self.0.fmt( formatter )
	}
}
