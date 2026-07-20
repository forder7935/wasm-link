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
use wasmtime::component::{ Component, Linker, Val };
use wasmtime::{ Engine, Store };

use crate::cardinality::Cardinality;
use crate::interface::{ Function as FunctionMetadata, Interface as InterfaceMetadata };
use crate::plugin::Plugin as PluginCore;
use crate::plugin_instance::PluginInstanceSync;
use crate::runtime_binding::define_runtime_bindings;
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

define_runtime_bindings!(
	PluginInstanceSync,
	"A binding in the synchronous runtime.",
	"A synchronous binding with erased cardinality."
);

impl<Id, Ctx, Plugins> Binding<Id, Ctx, Plugins>
where
	Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
	Ctx: PluginContext + 'static,
	Plugins: Cardinality<Id, PluginInstanceSync<Ctx>> + 'static,
	PluginSockets<Id, Ctx, Plugins>: Cardinality<
		Id,
		std::sync::Arc<futures::lock::Mutex<PluginInstanceSync<Ctx>>>,
	> + Send + Sync,
{
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
