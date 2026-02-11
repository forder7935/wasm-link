use std::sync::Arc ;
use std::collections::{ HashMap, HashSet };
use wasmtime::component::{ Linker, ResourceType, Val };

use crate::{ Binding, PluginContext };
use crate::linker::{ dispatch_all, dispatch_method };
use crate::resource_wrapper::ResourceWrapper ;

/// A single WIT interface within a [`Binding`].
///
/// Each interface declares functions and resources that implementers must export.
/// Note that the interface name is not a part of the struct but rather a key in
/// a hash map provided to the Binding constructor. This is to prevent duplicate
/// interface names.
///
/// ```
/// # use std::collections::{ HashMap, HashSet };
/// # use wasm_link::{ Binding, Interface, Socket };
/// # struct Ctx { resource_table: wasm_link::ResourceTable }
/// # impl wasm_link::PluginContext for Ctx {
/// #     fn resource_table( &mut self ) -> &mut wasm_link::ResourceTable { &mut self.resource_table }
/// # }
/// # fn example<T>( plugin: wasm_link::Engine ) -> Binding<String, Ctx> where T: wasm_link::PluginContext {
/// Binding::new(
///     "my:package",
///     HashMap::from([
///         ( "interface-a".to_string(), Interface::new( HashMap::new(), HashSet::new() )),
///         ( "interface-b".to_string(), Interface::new( HashMap::new(), HashSet::new() )),
///     ]),
///     Socket::AtMostOne( None ),
/// )
/// # }
/// ```
#[derive( Debug, Clone, Default )]
pub struct Interface {
    /// Functions exported by this interface
    functions: HashMap<String, Function>,
    /// Resource types defined by this interface
    resources: HashSet<String>,
}

impl Interface {
    /// Creates a new interface declaration.
    pub fn new(
        functions: HashMap<String, Function>,
        resources: HashSet<String>,
    ) -> Self {
        Self { functions, resources }
    }

    #[inline]
    pub(crate) fn function( &self, name: &str ) -> Option<&Function> {
        self.functions.get( name )
    }

    #[inline]
    pub(crate) fn add_to_linker<PluginId, Ctx>(
        &self,
        linker: &mut Linker<Ctx>,
        interface_ident: &str,
        binding: &Binding<PluginId, Ctx>,
    ) -> Result<(), wasmtime::Error>
    where
        PluginId: std::hash::Hash + Eq + Clone + Send + Sync + Into<Val> + 'static,
        Ctx: PluginContext,
    {
        let mut linker_root = linker.root();
        let mut linker_instance = linker_root.instance( interface_ident )?;

        self.functions.iter().try_for_each(|( name, metadata )| {

            let interface_ident_clone = interface_ident.to_string();
            let binding_clone = binding.clone();
            let name_clone = name.clone();
            let metadata_clone = metadata.clone();

            macro_rules! link {( $dispatch: expr ) => {
                linker_instance.func_new( name, move | ctx, _ty, args, results | Ok(
                    results[0] = $dispatch( &binding_clone, ctx, &interface_ident_clone, &name_clone, &metadata_clone, args )
                ))
            }}

            match metadata.is_method() {
                false => link!( dispatch_all ),
                true => link!( dispatch_method ),
            }

        })?;

        self.resources.iter().try_for_each(| resource | linker_instance
            .resource( resource.as_str(), ResourceType::host::<Arc<ResourceWrapper<PluginId>>>(), ResourceWrapper::<PluginId>::drop )
        )?;

        Ok(())

    }

}

/// Metadata about a function declared by an interface.
///
/// Provides information needed during linking to wire up cross-plugin dispatch.
/// Functions can also specify resource limits (fuel and epoch deadlines) that
/// constrain execution when dispatching to plugins. These limits may be modified
/// for each plugin separately by either a multiplier or specific overrides. If
/// no limit is specified, the [`Binding`] default is used.
///
/// # Resource Limits
///
/// Fuel and epoch deadlines prevent runaway or malicious plugins from consuming
/// unlimited resources.
///
/// ```
/// use wasm_link::{ Function, ReturnKind };
///
/// // A function with a fuel limit of 10,000 instructions
/// let compute_fn = Function::new( ReturnKind::AssumeNoResources, false )
///     .with_fuel( 10_000 );
///
/// // A function with an epoch deadline of 5 ticks
/// let io_fn = Function::new( ReturnKind::AssumeNoResources, false )
///     .with_epoch_deadline( 5 );
/// ```
#[derive( Debug, Clone )]
pub struct Function {
    /// The function's return kind for dispatch handling
    return_kind: ReturnKind,
    /// Whether this function is a method (has a `self` parameter)
    ///
    /// Methods route to the specific plugin that created the resource,
    /// rather than broadcasting to all plugins.
    is_method: bool,
    /// Fuel limit for this specific function (overrides binding default)
    fuel: Option<u64>,
    /// Epoch deadline for this specific function (overrides binding default)
    epoch_deadline: Option<u64>,
}

impl Function {
    /// Creates a new function metadata entry.
    pub fn new(
        return_kind: ReturnKind,
        is_method: bool,
    ) -> Self {
        Self { return_kind, is_method, fuel: None, epoch_deadline: None }
    }

    /// Sets the fuel limit for this function.
    ///
    /// Fuel limits bound how many WebAssembly instructions a function can execute.
    /// When fuel runs out, execution traps with a [`RuntimeException`]( crate::DispatchError::RuntimeException ).
    ///
    /// This value overrides the binding's default fuel. Plugins can further modify
    /// this via multipliers or per-function overrides.
    ///
    /// Fuel and epoch limits are independent—a function can have both, and whichever
    /// is exhausted first causes a trap.
    ///
    /// **Warning:** Fuel consumption must be enabled in the [`Engine`]( wasmtime::Engine )
    /// via [`Config::consume_fuel`]( wasmtime::Config::consume_fuel ). If not enabled,
    /// dispatch will fail with a [`RuntimeException`]( crate::DispatchError::RuntimeException )
    /// at call time, not at setup time.
    ///
    /// Look at [`wasmtime`] documentation for more detail.
    pub fn with_fuel( mut self, fuel: u64 ) -> Self {
        self.fuel = Some( fuel );
        self
    }

    /// Sets the epoch deadline for this function.
    ///
    /// Epoch deadlines bound wall-clock time by counting "ticks" incremented by an
    /// external timer. When the deadline is reached, execution traps with a
    /// [`RuntimeException`]( crate::DispatchError::RuntimeException ).
    ///
    /// This value overrides the binding's default epoch deadline. Plugins can further
    /// modify this via multipliers or per-function overrides.
    ///
    /// Fuel and epoch limits are independent—a function can have both, and whichever
    /// is exhausted first causes a trap.
    ///
    /// **Warning:** Epoch interruption must be enabled in the [`Engine`]( wasmtime::Engine )
    /// via [`Config::epoch_interruption`]( wasmtime::Config::epoch_interruption ). If not
    /// enabled, the deadline is silently ignored.
    ///
    /// Look at [`wasmtime`] documentation for more detail.
    pub fn with_epoch_deadline( mut self, ticks: u64 ) -> Self {
        self.epoch_deadline = Some( ticks );
        self
    }

    /// The function's return kind for dispatch handling.
    pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

    /// Whether this function is a method (has a `self` parameter).
    pub fn is_method( &self ) -> bool { self.is_method }

    /// The fuel limit for this function, if set.
    pub fn fuel( &self ) -> Option<u64> { self.fuel }

    /// The epoch deadline for this function, if set.
    pub fn epoch_deadline( &self ) -> Option<u64> { self.epoch_deadline }

}

/// Categorizes a function's return for dispatch handling.
///
/// Determines how return values are processed during cross-plugin dispatch.
/// Resources require special wrapping to track ownership across plugin
/// boundaries, while plain data can be passed through directly.
///
/// # Choosing the Right Variant
///
/// **When uncertain, use [`MayContainResources`](Self::MayContainResources).** Using
/// `AssumeNoResources` when resources are actually present will cause resource handles
/// to be passed through unwrapped causing runtime exceptions.
///
/// `AssumeNoResources` is a performance optimization that skips the wrapping step.
/// Only use it when you are certain the return type contains no resource handles
/// anywhere in its structure (including nested within records, variants, lists, etc.).
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
