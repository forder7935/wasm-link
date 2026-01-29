//! Plugin dependency tree construction.
//!
//! The [`PluginTree`] represents an unloaded plugin dependency tree. Internally,
//! multiple plugins may share a dependency (so it's technically a DAG), but this
//! is an implementation detail - conceptually it's a tree rooted at the entry
//! interface, and cycles are forbidden.
//!
//! Call [`PluginTree::load`] to compile the WASM components and link them together.

use std::collections::HashMap ;
use itertools::Itertools ;
use thiserror::Error ;
use wasmtime::Engine ;
use wasmtime::component::Linker ;

use crate::interface::{ InterfaceId, InterfaceData };
use crate::plugin::PluginData ;
use crate::plugin_tree_head::PluginTreeHead ;
use crate::loading::{ LoadError, load_plugin_tree };
use crate::utils::{ PartialSuccess, PartialResult, Merge };



/// Error that can occur during plugin tree construction.
///
/// These errors occur in [`PluginTree::new`] when building the dependency graph,
/// before any WASM compilation happens.
#[derive( Debug, Error )]
pub enum PluginTreeError<P: PluginData> {
    /// Failed to read plugin metadata (e.g., couldn't determine the plugin's plug).
    PluginDataError( P::Error ),
    /// Plugins reference an interface that wasn't provided in the interfaces list.
    MissingInterface { interface_id: InterfaceId, plugins: Vec<P> },
}

impl<P: PluginData> std::fmt::Display for PluginTreeError<P> {
    fn fmt( &self, f: &mut std::fmt::Formatter<'_> ) -> std::fmt::Result {
        match self {
            Self::PluginDataError( e ) => write!( f, "Plugin data error: {}", e ),
            Self::MissingInterface { interface_id, plugins } =>
                write!( f, "Missing interface {} required by {} plugins", interface_id, plugins.len())
        }
    }
}



/// An unloaded plugin dependency tree.
///
/// Built from a list of plugins by grouping them according to the interfaces
/// they implement (their plug) and depend on (their sockets). The structure
/// has a single root interface and cycles are forbidden, so it can be thought
/// of as a tree (though internally, multiple plugins may share a dependency).
///
/// This is the pre-compilation representation - no WASM has been loaded yet.
///
/// Call [`load`](Self::load) to compile WASM components and link dependencies,
/// producing a [`PluginTreeHead`] for dispatching function calls.
///
/// # Type Parameters
/// - `I`: [`InterfaceData`] implementation for loading interface metadata
/// - `P`: [`PluginData`] implementation for loading plugin metadata
///
/// # Example
/// ```ignore
/// // Build the unloaded dependency graph
/// let (tree, errors) = PluginTree::new(
///     root_interface_id,
///     interfaces,
///     plugins,
/// );
///
/// // Handle any build errors (e.g., missing interfaces)
/// for error in &errors {
///     eprintln!("Warning: {}", error);
/// }
///
/// // Compile and link
/// let tree_head = tree.load(&engine, &linker)?;
/// ```
pub struct PluginTree<I: InterfaceData, P: PluginData> {
    root_interface_id: InterfaceId,
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
}

impl<I: InterfaceData, P: PluginData> PluginTree<I, P> {

    /// Builds a plugin dependency graph from the given interfaces and plugins.
    ///
    /// Plugins are grouped by the interface they implement ( via `get_plug()` ).
    /// Interfaces are indexed by their `id()` method.
    ///
    /// The `root_interface_id` specifies the entry point of the tree - the interface
    /// whose plugins will be directly accessible via [`PluginTreeHead::dispatch`] after loading.
    ///
    /// Does not validate all interfaces required for linking are present.
    /// Does not validate cardinality requirements.
    ///
    /// # Partial Success
    /// Attempts to construct a tree for all plugins it received valid data for. Returns a list
    /// of errors alongside the loaded `PluginTree` is any of the following occurs:
    /// - An Interface mentioned in a plugin's plug is not passed in
    /// - Calling [`PluginData::get_plug`] returns an error
    ///
    /// # Panics
    /// Panics if an interface with id `root_interface_id` is not present in `interfaces`.
    pub fn new(
        root_interface_id: InterfaceId,
        interfaces: impl IntoIterator<Item = I>,
        plugins: impl IntoIterator<Item = P>,
    ) -> PartialSuccess<Self, PluginTreeError<P>> {

        let interface_map = interfaces.into_iter()
            .map(| i | ( i.id(), i ))
            .collect::<HashMap<_, _ >>();

        assert!(
            interface_map.contains_key( &root_interface_id ),
            "Root interface {} must be provided in interfaces list",
            root_interface_id,
        );

        let ( entries, plugin_errors ) = plugins.into_iter()
            .map(| plugin | Ok(( *plugin.get_plug().map_err( PluginTreeError::PluginDataError )?, plugin )))
            .partition_result::<Vec<_>, Vec<_>, _, _>();

        let plugin_groups = entries.into_iter().into_group_map();
        let mut interface_map = interface_map ;

        let ( socket_entries, missing_errors ) = plugin_groups.into_iter()
            .map(|( id, plugins )| match interface_map.remove( &id ) {
                Some( interface ) => Ok(( id, ( interface, plugins ))),
                None => Err( PluginTreeError::MissingInterface { interface_id: id, plugins }),
            })
            .partition_result::<Vec<_>, Vec<_>, _, _>();

        // Include remaining interfaces with no plugins. Does not overwrite any
        // entries since interfaces for sockets that had plugins on them were
        // already removed from the map.
        let socket_map = socket_entries.into_iter()
            .chain( interface_map.into_iter().map(|( id, interface )| ( id, ( interface, Vec::new() ))))
            .collect::<HashMap<_, _>>();

        ( Self { root_interface_id, socket_map }, plugin_errors.merge_all( missing_errors ))

    }

    /// Creates a plugin tree directly from a pre-built socket map.
    ///
    /// Does not validate all interfaces required for linking are present.
    /// Does not validate cardinality requirements.
    ///
    /// # Panics
    /// Panics if an interface with id `root_interface_id` is not present in `interfaces`.
    pub fn from_socket_map(
        root_interface_id: InterfaceId,
        socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
    ) -> Self {

        assert!(
            socket_map.contains_key( &root_interface_id ),
            "Root interface {} must be provided in interfaces list",
            root_interface_id,
        );

        Self { root_interface_id, socket_map }
    }

    /// Compiles and links all plugins in the tree, returning a loaded tree head.
    ///
    /// This recursively loads plugins starting from the root interface, compiling
    /// WASM components and linking their dependencies.
    ///
    /// # Errors
    /// Returns `LoadError` variants for:
    /// - Invalid or missing socket interfaces
    /// - Dependency cycles between plugins
    /// - Cardinality violations (too few/many plugins for an interface)
    /// - Corrupted interface or plugin manifests
    /// - WASM compilation or linking failures
    pub fn load(
        self,
        engine: &Engine,
        exports: &Linker<P>,
    ) -> PartialResult<PluginTreeHead<I, P>, LoadError<I, P>, LoadError<I, P>>
    where
        P: Send + Sync,
    {
        match load_plugin_tree( self.socket_map, engine, exports, self.root_interface_id ) {
            Ok((( interface, socket ), errors )) => Ok(( PluginTreeHead { _interface: interface, socket }, errors )),
            Err(( err, errors )) => Err(( err , errors )),
        }
    }

}
