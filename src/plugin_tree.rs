//! Plugin dependency tree construction.
//!
//! The [`PluginTree`] represents an unloaded plugin dependency tree. Multiple plugins
//! may share a dependency (so it's technically a graph), though cycles are forbidden.
//!
//! Call [`PluginTree::load`] to compile the WASM components and link them together.

use std::collections::HashMap ;
use itertools::Itertools ;
use thiserror::Error ;
use wasmtime::Engine ;
use wasmtime::component::Linker ;

use crate::interface::Binding ;
use crate::plugin::{ Plugin, PluginContext } ;
use crate::plugin_tree_head::PluginTreeHead ;
use crate::loading::{ LoadError, load_plugin_tree };
use crate::utils::PartialResult ;

/// Type alias for the socket map used in plugin tree construction.
type SocketMap<BindingId, PluginId, Ctx> = HashMap<BindingId, ( Binding<BindingId>, Vec<Plugin<PluginId, BindingId, Ctx>> )>;



/// Error that can occur during plugin tree construction.
///
/// These errors occur in [`PluginTree::new`] when building the dependency graph,
/// before any WASM compilation happens.
#[derive( Debug, Error )]
pub enum PluginTreeError<BindingId, PluginId>
where
    BindingId: std::fmt::Display,
    PluginId: std::fmt::Display,
{
    /// Plugins reference an interface binding that wasn't provided in the bindings list.
    #[error( "Missing interface binding {} required by {} plugins", binding_id, plugins.len() )]
    MissingBinding { binding_id: BindingId, plugins: Vec<PluginId> },
}



/// An unloaded plugin dependency tree.
///
/// Built from a list of plugins by grouping them according to the interface bindings
/// they implement (their plug) and depend on (their sockets). The structure
/// has a single root binding, shared dependencies are allowed but cycles are forbidden.
///
/// This is the pre-compilation representation - no WASM has been loaded yet.
///
/// Call [`load`]( Self::load ) to instantiate WASM components and link dependencies,
/// producing a [`PluginTreeHead`] for dispatching function calls.
///
/// # Type Parameters
/// - `BindingId`: Identifier type for interface bindings
/// - `PluginId`: Identifier type for plugins
/// - `Ctx`: User context type stored in wasmtime Store
pub struct PluginTree<BindingId, PluginId, Ctx> {
    root_binding_id: BindingId,
    socket_map: SocketMap<BindingId, PluginId, Ctx>,
}

impl<BindingId, PluginId, Ctx> PluginTree<BindingId, PluginId, Ctx>
where
    BindingId: Clone + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Display + std::fmt::Debug,
    PluginId: Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + std::fmt::Display + std::fmt::Debug + 'static,
    Ctx: PluginContext + 'static,
{

    /// Builds a plugin dependency graph from the given interface bindings and plugins.
    ///
    /// Plugins are grouped by the binding they implement (via `Plugin.plug`).
    /// Bindings are indexed by their `id` field.
    ///
    /// The `root_binding_id` specifies the entry point of the tree - the binding
    /// whose plugins will be directly accessible via [`PluginTreeHead::dispatch`] after loading.
    ///
    /// Does not validate all bindings required for linking are present.
    /// Does not validate cardinality requirements.
    ///
    /// # Partial Success
    /// Returns a list of errors for plugins whose plug binding wasn't provided.
    ///
    /// # Panics
    /// Panics if a binding with id `root_binding_id` is not present in `bindings`.
    pub fn new(
        root_binding_id: impl Into<BindingId>,
        bindings: impl IntoIterator<Item = Binding<BindingId>>,
        plugins: impl IntoIterator<Item = Plugin<PluginId, BindingId, Ctx>>,
    ) -> ( Self, Vec<PluginTreeError<BindingId, PluginId>> ) {

        let root_binding_id = root_binding_id.into();

        let bindings = bindings.into_iter()
            .map(| binding | ( binding.id().clone(), binding ))
            .collect::<HashMap<_, _>>();

        assert!(
            bindings.contains_key( &root_binding_id ),
            "Root binding {} must be provided in bindings list",
            root_binding_id,
        );

        let plugin_groups = plugins.into_iter()
            .map(| plugin | ( plugin.plug().clone(), plugin ))
            .into_group_map();

        let mut bindings = bindings ;

        let ( socket_entries, errors ) = plugin_groups.into_iter()
            .map(|( id, plugins )| match bindings.remove( &id ) {
                Some( interface ) => Ok(( id, ( interface, plugins ))),
                None => Err( PluginTreeError::MissingBinding {
                    binding_id: id,
                    plugins: plugins.iter()
                        .map(| plugin | plugin.id().clone())
                        .collect()
                }),
            })
            .partition_result::<Vec<_>, Vec<_>, _, _>();

        // Include remaining bindings with no plugins. Does not overwrite any
        // entries since bindings for sockets that had plugins on them were
        // already removed from the map.
        let socket_map = socket_entries.into_iter()
            .chain( bindings.into_iter().map(|( id, binding )| ( id, ( binding, Vec::with_capacity( 0 ) ))))
            .collect::<HashMap<_, _>>();

        ( Self { root_binding_id, socket_map }, errors )

    }

    /// Creates a plugin tree directly from a pre-built socket map.
    ///
    /// Does not validate all bindings required for linking are present.
    /// Does not validate cardinality requirements.
    ///
    /// # Panics
    /// Panics if a binding with id `root_binding_id` is not present in `socket_map`.
    pub fn from_hash_map(
        root_binding_id: impl Into<BindingId>,
        socket_map: SocketMap<BindingId, PluginId, Ctx>,
    ) -> Self {

        let root_binding_id = root_binding_id.into();

        assert!(
            socket_map.contains_key( &root_binding_id ),
            "Root binding {} must be provided in socket map",
            root_binding_id,
        );

        Self { root_binding_id, socket_map }

    }

    /// Compiles and links all plugins in the tree, returning a loaded tree head.
    ///
    /// This recursively loads plugins starting from the root binding, instantiating
    /// WASM components and linking their dependencies.
    ///
    /// # Errors
    /// Returns `LoadError` variants for:
    /// - Invalid or missing socket bindings
    /// - Dependency cycles between plugins
    /// - Cardinality violations (too few/many plugins for a binding)
    /// - WASM instantiation or linking failures
    pub fn load(
        self,
        engine: &Engine,
        exports: &Linker<Ctx>,
    ) -> PartialResult<PluginTreeHead<BindingId, PluginId, Ctx>, LoadError<BindingId>> {
        match load_plugin_tree( self.socket_map, engine, exports, self.root_binding_id ) {
            Ok((( binding, socket ), errors )) => Ok(( PluginTreeHead { binding, socket }, errors )),
            Err(( err, errors )) => Err(( err , errors )),
        }
    }

}
