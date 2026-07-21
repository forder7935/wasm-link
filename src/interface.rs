use std::sync::Arc ;
use std::collections::{ HashMap, HashSet };
use futures::lock::Mutex ;
use wasmtime::component::{ Linker, ResourceType, Val };

use crate::binding::Binding;
use crate::plugin::PluginContext;
use crate::plugin_instance::{ concurrent, sync };
use crate::cardinality::Cardinality ;
use crate::linker::{
	dispatch_all,
	dispatch_all_async,
	dispatch_all_async_blocking,
	dispatch_method,
	dispatch_method_async,
	dispatch_method_async_blocking,
};
use crate::resource_wrapper::ResourceWrapper ;

/// A single WIT interface within a binding.
///
/// Each interface declares functions and resources that implementers must export.
/// Note that the interface name is not a part of the struct but rather a key in
/// a hash map provided to the Binding constructor. This is to prevent duplicate
/// interface names.
///
/// ```
/// # use std::collections::{ HashMap, HashSet };
/// # use wasm_link::sync::{ Binding, Interface, PluginInstance };
/// # use wasm_link::{ PluginContext, ResourceTable };
/// # use wasm_link::cardinality::AtMostOne ;
/// # struct Ctx { resource_table: ResourceTable }
/// # impl PluginContext for Ctx {
/// # 	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
/// # }
/// let binding: Binding<String, Ctx, AtMostOne<String, PluginInstance<Ctx>>> = Binding::new(
/// 	"my:package",
/// 	HashMap::from([
/// 		( "interface-a".to_string(), Interface::new( HashMap::<String, wasm_link::sync::Function>::new(), HashSet::new() )),
/// 		( "interface-b".to_string(), Interface::new( HashMap::<String, wasm_link::sync::Function>::new(), HashSet::new() )),
/// 	]),
/// 	AtMostOne( None ),
/// );
/// # let _ = binding;
/// ```
#[derive( Debug, Clone, Default )]
pub struct Interface {
	/// Functions exported by this interface
	functions: HashMap<String, FunctionMetadata>,
	/// Resource types defined by this interface
	resources: HashSet<String>,
}

impl Interface {
	/// Creates a new interface declaration.
	pub fn new<FunctionSpec>(
		functions: HashMap<String, FunctionSpec>,
		resources: HashSet<String>,
	) -> Self
	where
		FunctionSpec: Into<FunctionMetadata>,
	{
		Self {
			functions: functions.into_iter()
				.map(|( name, function )| ( name, function.into() ))
				.collect(),
			resources,
		}
	}

	#[inline]
	pub(crate) fn function( &self, name: &str ) -> Option<&FunctionMetadata> {
		self.functions.get( name )
	}

	#[inline]
	pub(crate) fn add_to_linker<PluginId, Ctx, Plugins>(
		&self,
		linker: &mut Linker<Ctx>,
		package_name: &str,
		interface_ident: &str,
		interface_name: &str,
		binding: &Binding<PluginId, Ctx, Plugins, sync::PluginInstance<Ctx>>,
	) -> Result<(), wasmtime::Error>
	where
		PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
		Ctx: PluginContext,
		Plugins: Cardinality<PluginId, sync::PluginInstance<Ctx>> + 'static,
		<Plugins as Cardinality<PluginId, sync::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<sync::PluginInstance<Ctx>>>>: Send + Sync,
		<Plugins as Cardinality<PluginId, sync::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<sync::PluginInstance<Ctx>>>>: Cardinality<PluginId, Arc<Mutex<sync::PluginInstance<Ctx>>>>,
		<<Plugins as Cardinality<PluginId, sync::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<sync::PluginInstance<Ctx>>>> as Cardinality<PluginId, Arc<Mutex<sync::PluginInstance<Ctx>>>>>::Rebind<Val>: Into<Val>,
	{
		let mut linker_root = linker.root();
		let mut linker_instance = linker_root.instance( interface_ident )?;

		self.functions.iter().try_for_each(|( name, metadata )| {

			let package_name_clone = package_name.to_string();
			let interface_name_clone = interface_name.to_string();
			let binding_clone = binding.clone();
			let name_clone = name.clone();
			let metadata_clone = metadata.clone();

			macro_rules! link {( $dispatch: expr ) => {
				linker_instance.func_new( name, move | ctx, _ty, args, results | Ok(
					results[0] = $dispatch( &binding_clone, ctx, &package_name_clone, &interface_name_clone, &name_clone, &metadata_clone, args )
				))
			}}

			match metadata.kind() {
				FunctionKind::Freestanding => link!( dispatch_all ),
				FunctionKind::Method => link!( dispatch_method ),
			}

		})?;

		self.resources.iter().try_for_each(| resource | linker_instance
			.resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper<PluginId>>>(), ResourceWrapper::<PluginId>::drop )
		)?;

		Ok(())

	}

	#[inline]
	pub(crate) fn add_to_linker_async<PluginId, Ctx, Plugins>(
		&self,
		linker: &mut Linker<Ctx>,
		package_name: &str,
		interface_ident: &str,
		interface_name: &str,
		binding: &Binding<PluginId, Ctx, Plugins, concurrent::PluginInstance<Ctx>>,
	) -> Result<(), wasmtime::Error>
	where
		PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
		Ctx: PluginContext,
		Plugins: Cardinality<PluginId, concurrent::PluginInstance<Ctx>> + 'static,
		<Plugins as Cardinality<PluginId, concurrent::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<concurrent::PluginInstance<Ctx>>>>: Send + Sync,
		<Plugins as Cardinality<PluginId, concurrent::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<concurrent::PluginInstance<Ctx>>>>: Cardinality<PluginId, Arc<Mutex<concurrent::PluginInstance<Ctx>>>>,
		<<Plugins as Cardinality<PluginId, concurrent::PluginInstance<Ctx>>>::Rebind<Arc<Mutex<concurrent::PluginInstance<Ctx>>>> as Cardinality<PluginId, Arc<Mutex<concurrent::PluginInstance<Ctx>>>>>::Rebind<Val>: Into<Val> + Send,
	{
		let mut linker_root = linker.root();
		let mut linker_instance = linker_root.instance( interface_ident )?;

		self.functions.iter().try_for_each(|( name, metadata )| {
			let package_name = package_name.to_string();
			let interface_name = interface_name.to_string();
			let binding = binding.clone();
			let function_name = name.clone();
			let function = metadata.clone();

			macro_rules! link_concurrent {( $dispatch: expr ) => {
				linker_instance.func_new_concurrent( name, move | ctx, _ty, args, results | {
					let package_name = package_name.clone();
					let interface_name = interface_name.clone();
					let binding = binding.clone();
					let function_name = function_name.clone();
					let function = function.clone();
					Box::pin( async move {
						results[0] = $dispatch(
							&binding, ctx, &package_name, &interface_name, &function_name, &function, args,
						).await;
						Ok(())
					})
				})
			}}

			macro_rules! link_blocking {( $dispatch: expr ) => {
				linker_instance.func_new_async( name, move | ctx, _ty, args, results | {
					let package_name = package_name.clone();
					let interface_name = interface_name.clone();
					let binding = binding.clone();
					let function_name = function_name.clone();
					let function = function.clone();
					Box::new( async move {
						results[0] = $dispatch(
							&binding, ctx, &package_name, &interface_name, &function_name, &function, args,
						).await;
						Ok(())
					})
				})
			}}

			match ( metadata.is_async(), metadata.kind() ) {
				( true, FunctionKind::Freestanding ) => link_concurrent!( dispatch_all_async ),
				( true, FunctionKind::Method ) => link_concurrent!( dispatch_method_async ),
				( false, FunctionKind::Freestanding ) => link_blocking!( dispatch_all_async_blocking ),
				( false, FunctionKind::Method ) => link_blocking!( dispatch_method_async_blocking ),
			}
		})?;

		self.resources.iter().try_for_each(| resource | linker_instance.resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper<PluginId>>>(), ResourceWrapper::<PluginId>::drop ))?;

		Ok(())
	}

}

/// Denotes whether a function is freestanding or a resource method.
/// Constructors are treated as freestanding functions.
///
/// Determines how dispatch is routed during cross-plugin calls:
/// freestanding functions broadcast to all plugins, while methods
/// route to the specific plugin that owns the resource.
#[derive( Debug, Clone, Copy, Eq, PartialEq )]
pub enum FunctionKind {
	/// A freestanding function — dispatched to all plugins.
	Freestanding,
	/// A resource method (has a `self` parameter) — routed to the plugin that owns the resource.
	Method,
}

/// Metadata about a function declared by an interface.
///
/// Provides information needed during linking to wire up cross-plugin dispatch.
#[derive( Debug, Clone )]
pub struct FunctionMetadata {
	/// Whether this function is freestanding or a resource method.
	kind: FunctionKind,
	/// The function's return kind for dispatch handling
	return_kind: ReturnKind,
	/// Whether the WIT function is declared with the `async` effect.
	is_async: bool,
}

impl FunctionMetadata {
	/// Creates a new function metadata entry.
	pub fn new(
		kind: FunctionKind,
		return_kind: ReturnKind,
	) -> Self {
		Self { kind, return_kind, is_async: false }
	}

	/// Creates metadata for a WIT function declared with the `async` effect.
	///
	/// ```
	/// use wasm_link::concurrent::Function;
	/// use wasm_link::{ FunctionKind, ReturnKind };
	///
	/// let function = Function::new_async(
	/// 	FunctionKind::Freestanding,
	/// 	ReturnKind::MayContainResources,
	/// );
	/// assert!( function.is_async() );
	/// ```
	pub fn new_async(
		kind: FunctionKind,
		return_kind: ReturnKind,
	) -> Self {
		Self { kind, return_kind, is_async: true }
	}

	/// The function's return kind for dispatch handling.
	pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

	/// Whether this function is freestanding or a resource method.
	pub fn kind( &self ) -> FunctionKind { self.kind }

	/// Whether the WIT function is declared with the `async` effect.
	///
	/// ```
	/// # use wasm_link::concurrent::Function;
	/// # use wasm_link::{ FunctionKind, ReturnKind };
	/// let function = Function::new( FunctionKind::Freestanding, ReturnKind::Void );
	/// assert!( !function.is_async() );
	/// ```
	pub fn is_async( &self ) -> bool { self.is_async }

}

/// Categorizes a function's return for dispatch handling.
///
/// Determines how return values are processed during cross-plugin dispatch.
/// Resources require special wrapping to track ownership across plugin
/// boundaries, while plain data can be passed through directly.
///
/// # Choosing the Right Variant
///
/// **When uncertain, use [`MayContainResources`]( Self::MayContainResources ).**
/// Using [`AssumeNoResources`]( Self::AssumeNoResources ) when resources are
/// actually present will cause resource handles to be passed through unwrapped
/// causing runtime exceptions.
///
/// [`AssumeNoResources`]( Self::AssumeNoResources ) is a performance optimization
/// that skips the wrapping step. Only use it when you are certain the return type
/// contains no resource handles anywhere in its structure (including nested within
/// records, variants, lists, etc.).
#[derive( Copy, Clone, Eq, PartialEq, Hash, Debug, Default )]
pub enum ReturnKind {
	/// Function returns nothing (void).
	#[default] Void,
	/// Function may return resource handles - always wraps safely.
	///
	/// Use this variant whenever resources might be present in the return value,
	/// or when you're unsure. The performance overhead of wrapping is preferable
	/// to the undefined behavior caused by unwrapped resource handles.
	MayContainResources,
	/// Assumes no resource handles are present - skips wrapping for performance.
	///
	/// **Warning:** Only use this if you are certain no resources are present.
	/// If resources are returned but this variant is used, resource handles will
	/// not be wrapped correctly, potentially causing undefined behavior in plugins.
	/// When in doubt, use [`MayContainResources`](Self::MayContainResources) instead.
	AssumeNoResources,
}

impl std::fmt::Display for ReturnKind {
	fn fmt( &self, f: &mut std::fmt::Formatter ) -> Result<(), std::fmt::Error> {
		match self {
			Self::Void => write!( f, "Function returns no data" ),
			Self::MayContainResources => write!( f, "Return type may contain resources" ),
			Self::AssumeNoResources => write!( f, "Function is assumed to not return any resources" ),
		}
	}
}
