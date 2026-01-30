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
pub enum PluginTreeError<I: InterfaceData, P: PluginData> {
    /// Failed to read interface metadata (e.g., couldn't determine the interface's id).
    #[error( "InterfaceData error: {0}" )] InterfaceDataError( I::Error ),
    /// Failed to read plugin metadata (e.g., couldn't determine the plugin's plug).
    #[error( "PluginData error: {0}" )] PluginDataError( P::Error ),
    /// Plugins reference an interface that wasn't provided in the interfaces list.
    #[error( "Missing interface {} required by {} plugins", interface_id, plugins.len() )]
    MissingInterface { interface_id: InterfaceId, plugins: Vec<P> },
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
/// Call [`load`]( Self::load ) to compile WASM components and link dependencies,
/// producing a [`PluginTreeHead`] for dispatching function calls.
///
/// # Type Parameters
/// - `I`: [`InterfaceData`] implementation for loading interface metadata
/// - `P`: [`PluginData`] implementation for loading plugin metadata
///
/// # Example
///
/// ```
/// use wasm_compose::{
///     InterfaceId, InterfaceData, InterfaceCardinality, FunctionData, ReturnKind,
///     PluginId, PluginData, PluginTree, Engine, Component, Linker,
/// };
///
/// # #[derive( Clone )]
/// # struct Func { name: &'static str, return_kind: ReturnKind }
/// # impl FunctionData for Func {
/// #   fn name( &self ) -> &str { unreachable!() }
/// #   fn return_kind( &self ) -> ReturnKind { unreachable!() }
/// #   fn is_method( &self ) -> bool { unreachable!() }
/// # }
/// #
/// struct Interface { id: InterfaceId, funcs: Vec<Func> }
/// impl InterfaceData for Interface {
///     /* ... */
/// #   type Error = std::convert::Infallible ;
/// #   type Function = Func ;
/// #   type FunctionIter<'a> = std::slice::Iter<'a, Func> ;
/// #   type ResourceIter<'a> = std::iter::Empty<&'a String> ;
/// #   fn id( &self ) -> Result<InterfaceId, Self::Error> { Ok( self.id ) }
/// #   fn cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> {
/// #       Ok( &InterfaceCardinality::ExactlyOne )
/// #   }
/// #   fn package_name( &self ) -> Result<&str, Self::Error> { Ok( "my:package/example" ) }
/// #   fn functions( &self ) -> Result<Self::FunctionIter<'_>, Self::Error> {
/// #       Ok( self.funcs.iter())
/// #   }
/// #   fn resources( &self ) -> Result<Self::ResourceIter<'_>, Self::Error> {
/// #       Ok( std::iter::empty())
/// #   }
/// }
///
/// struct Plugin { id: PluginId, plug: InterfaceId }
/// impl PluginData for Plugin {
///     /* ... */
/// #   type Error = std::convert::Infallible ;
/// #   type SocketIter<'a> = std::iter::Empty<&'a InterfaceId> ;
/// #   fn id( &self ) -> Result<&PluginId, Self::Error> { Ok( &self.id ) }
/// #   fn plug( &self ) -> Result<&InterfaceId, Self::Error> { Ok( &self.plug ) }
/// #   fn sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> {
/// #       Ok( std::iter::empty())
/// #   }
/// #   fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> {
/// #       Ok( Component::new( engine, r#"(component
/// #           (core module $m)
/// #           (core instance $i (instantiate $m))
/// #           (instance $inst)
/// #           (export "my:package/example" (instance $inst))
/// #       )"# ).unwrap())
/// #   }
/// }
///
/// let root_interface_id = InterfaceId::new( 0 );
/// let plugins = [ Plugin { id: PluginId::new( 1 ), plug: root_interface_id }];
/// let interfaces = [ Interface { id: root_interface_id, funcs: vec![] }];
///
/// // Build the dependency graph
/// let ( tree, build_errors ) = PluginTree::new( root_interface_id, interfaces, plugins );
/// assert!( build_errors.is_empty() );
///
/// // Compile and link the plugins
/// let engine = Engine::default();
/// let linker = Linker::new( &engine );
/// let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
/// assert!( load_errors.is_empty() );
/// ```
pub struct PluginTree<I: InterfaceData, P: PluginData> {
    root_interface_id: InterfaceId,
    socket_map: HashMap<InterfaceId, ( I, Vec<P> )>,
}

impl<I: InterfaceData, P: PluginData> PluginTree<I, P> {

    /// Builds a plugin dependency graph from the given interfaces and plugins.
    ///
    /// Plugins are grouped by the interface they implement ( via [`PluginData::plug`] ).
    /// Interfaces are indexed by their [`PluginData::id`] method.
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
    /// - Calling [`PluginData::plug`] returns an error
    ///
    /// # Panics
    /// Panics if an interface with id `root_interface_id` is not present in `interfaces`.
    pub fn new(
        root_interface_id: InterfaceId,
        interfaces: impl IntoIterator<Item = I>,
        plugins: impl IntoIterator<Item = P>,
    ) -> PartialSuccess<Self, PluginTreeError<I, P>> {

        let ( interface_map, interface_errors ) = interfaces.into_iter()
            .map(| i | Ok::<_, PluginTreeError<I, P>>(( i.id().map_err( PluginTreeError::InterfaceDataError )?, i )))
            .partition_result::<HashMap<_, _ >, Vec<_>, _, _>();

        assert!(
            interface_map.contains_key( &root_interface_id ),
            "Root interface {} must be provided in interfaces list",
            root_interface_id,
        );

        let ( entries, plugin_errors ) = plugins.into_iter()
            .map(| plugin | Ok(( *plugin.plug().map_err( PluginTreeError::PluginDataError )?, plugin )))
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

        ( Self { root_interface_id, socket_map }, interface_errors.merge_all( plugin_errors ).merge_all( missing_errors ))

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
