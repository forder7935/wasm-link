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
pub struct Function {
    /// Whether this function is freestanding or a resource method.
    kind: FunctionKind,
    /// The function's return kind for dispatch handling
    return_kind: ReturnKind,
}

impl Function {
    /// Creates a new function metadata entry.
    pub fn new(
        kind: FunctionKind,
        return_kind: ReturnKind,
    ) -> Self {
        Self { kind, return_kind }
    }

    /// The function's return kind for dispatch handling.
    pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

    /// Whether this function is freestanding or a resource method.
    pub fn kind( &self ) -> FunctionKind { self.kind }

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
