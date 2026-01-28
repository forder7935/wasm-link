use std::collections::HashMap ;
use itertools::Itertools ;
use wasmtime::Engine ;
use wasmtime::component::Linker ;

use crate::interface::{ InterfaceId, InterfaceData };
use crate::plugin::PluginData ;
use crate::plugin_tree_head::PluginTreeHead ;
use crate::loading::{ LoadError, load_plugin_tree };
use crate::utils::{ Merge, PartialSuccess, PartialResult };



/// An unloaded plugin dependency graph.
///
/// Built from a list of plugins by grouping them according to the interfaces
/// they implement (their "plug") and depend on (their "sockets").
///
/// # Type Parameters
/// - `I`: [`InterfaceData`] implementation for loading interface metadata
/// - `P`: [`PluginData`] implementation for loading plugin metadata
///
/// # Example
/// ```ignore
/// let plugins = vec![
///     MyPluginData::new( "auth-provider" ),
///     MyPluginData::new( "logger" ),
/// ];
/// let ( tree, discovery_errors ) = PluginTree::<MyInterfaceData, _>::new::<MyError>(
///     plugins,
///     InterfaceId::new( 0 ),
/// );
/// ```
pub struct PluginTree<I: InterfaceData, P: PluginData> {
    root_interface_id: InterfaceId,
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
}

impl<I: InterfaceData, P: PluginData> PluginTree<I, P> {

    /// Builds a plugin dependency graph from the given plugins.
    ///
    /// Plugins are grouped by the interface they implement. The `root_interface_id`
    /// specifies the entry point of the tree - the interface whose plugins will be
    /// directly accessible via [`PluginTreeHead::dispatch`] after loading.
    ///
    /// The error type `E` must be convertible from both `I::Error` and `P::Error`,
    /// allowing unified error handling across interface and plugin metadata access.
    pub fn new<E>(
        plugins: Vec<P>,
        root_interface_id: InterfaceId,
    ) -> PartialSuccess<Self, E>
    where
        E: From<I::Error> + From<P::Error>,
    {
        let ( entries, plugin_errors ) = plugins.into_iter()
            .map(| handle | Result::<_, E>::Ok(( *handle.get_plug()?, handle )))
            .partition_result::<Vec<_>, Vec<_>, _, _>();

        let mut plugin_group_map = entries.into_iter().into_group_map();
        plugin_group_map.entry( root_interface_id ).or_default();

        let ( socket_map, interface_errors ) = plugin_group_map.into_iter()
            .map(|( id, plugins )| Ok(( id, ( I::new( id )?, plugins ))))
            .partition_result::<HashMap<_, _>, Vec<_>, _, _>();

        ( Self { root_interface_id, socket_map }, plugin_errors.merge_all( interface_errors ) )
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
