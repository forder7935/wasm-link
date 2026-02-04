//! Binding specification and metadata types.
//!
//! A [`Binding`] defines an abstract contract specifying what plugins must implement
//! (via plugs) or what they could depend on (via sockets). It bundles one or more WIT
//! [`Interface`]s under a single identifier.

/// An abstract contract specifying what plugins must implement (via plugs) or what
/// they could depend on (via sockets). It bundles one or more WIT [`Interface`]s
/// under a single identifier.
///
/// # Type Parameters
/// - `BindingId`: Unique identifier type for the spec (e.g., `String`, `Uuid`)
#[derive( Debug, Clone )]
pub struct Binding<BindingId> {
    /// Unique identifier for this interface specification
    id: BindingId,
    /// How many plugins may/must implement this specification
    cardinality: Cardinality,
    /// WIT package name (e.g., "my:package")
    package_name: String,
    /// The WIT interfaces in this specification
    interfaces: Vec<Interface>,
}

impl<BindingId> Binding<BindingId> {
    /// Creates a new binding specification.
    #[inline]
    pub fn new(
        id: BindingId,
        cardinality: Cardinality,
        package_name: impl Into<String>,
        interfaces: impl IntoIterator<Item = Interface>,
    ) -> Self {
        Self {
            id,
            cardinality,
            package_name: package_name.into(),
            interfaces: interfaces.into_iter().collect(),
        }
    }

    /// Unique identifier for this binding.
    #[inline] pub fn id( &self ) -> &BindingId { &self.id }

    /// How many plugins may/must implement this binding.
    #[inline] pub fn cardinality( &self ) -> Cardinality { self.cardinality }

    /// WIT package name (e.g., "my:package").
    #[inline] pub fn package_name( &self ) -> &str { &self.package_name }

    /// The WIT interfaces in this binding.
    #[inline] pub fn interfaces( &self ) -> &[Interface] { &self.interfaces }
}

/// A single WIT interface within a [`Binding`].
///
/// Each interface declares functions and resources that implementers must export.
#[derive( Debug, Clone )]
pub struct Interface {
    /// Interface name as declared in WIT (e.g., "counter", "math", "root")
    name: String,
    /// Functions exported by this interface
    functions: Vec<Function>,
    /// Resource types defined by this interface
    resources: Vec<String>,
}

impl Interface {
    /// Creates a new interface declaration.
    #[inline]
    pub fn new(
        name: impl Into<String>,
        functions: impl IntoIterator<Item = Function>,
        resources: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            functions: functions.into_iter().collect(),
            resources: resources.into_iter().map( Into::into ).collect(),
        }
    }

    /// Interface name as declared in WIT.
    #[inline] pub fn name( &self ) -> &str { &self.name }

    /// Functions exported by this interface.
    #[inline] pub fn functions( &self ) -> &[Function] { &self.functions }

    /// Resource types defined by this interface.
    #[inline] pub fn resources( &self ) -> &[String] { &self.resources }
}

/// Metadata about a function declared by an interface.
///
/// Provides information needed during linking to wire up cross-plugin dispatch.
#[derive( Debug, Clone )]
pub struct Function {
    /// Function name as defined per WIT standards
    /// (e.g., `get-value`, `[constructor]counter`, `[method]counter.increment`)
    name: String,
    /// The function's return kind for dispatch handling
    return_kind: ReturnKind,
    /// Whether this function is a method (has a `self` parameter)
    ///
    /// Methods route to the specific plugin that created the resource,
    /// rather than broadcasting to all plugins.
    is_method: bool,
}

impl Function {
    /// Creates a new function metadata entry.
    #[inline]
    pub fn new(
        name: impl Into<String>,
        return_kind: ReturnKind,
        is_method: bool,
    ) -> Self {
        Self { name: name.into(), return_kind, is_method }
    }

    /// Function name as defined per WIT standards.
    #[inline] pub fn name( &self ) -> &str { &self.name }

    /// The function's return kind for dispatch handling.
    #[inline] pub fn return_kind( &self ) -> ReturnKind { self.return_kind }

    /// Whether this function is a method (has a `self` parameter).
    #[inline] pub fn is_method( &self ) -> bool { self.is_method }
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
/// to be passed through unwrapped. This can lead to undefined behavior in plugins:
/// invalid handles, use-after-free, or calls dispatched to the wrong plugin.
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

/// Specifies how many plugins may or must implement a [`Binding`].
///
/// Cardinality expresses what the consumer of a binding expects:
///
/// - `ExactlyOne`: The consumer expects a single implementation. Dispatch returns
///   a single value directly.
///
/// - `AtMostOne`: The consumer can work with zero or one implementation. Dispatch
///   returns an `Option`.
///
/// - `AtLeastOne`: The consumer requires at least one implementation but can handle
///   multiple. Dispatch returns a collection.
///
/// - `Any`: The consumer doesn't care how many implementations exist (including zero).
///   Dispatch returns a collection. Useful for optional extension points.
///
/// The cardinality determines the [`Socket`] variant used at runtime and affects
/// how dispatch results are wrapped.
///
/// [`Socket`]: crate::Socket
#[derive( Debug, PartialEq, Eq, Copy, Clone )]
pub enum Cardinality {
    /// Zero or one plugin allowed. Dispatch returns `Option<T>`.
    AtMostOne,
    /// Exactly one plugin required. Dispatch returns `T` directly.
    ExactlyOne,
    /// One or more plugins required. Dispatch returns a collection.
    AtLeastOne,
    /// Zero or more plugins allowed. Dispatch returns a collection.
    Any,
}
impl std::fmt::Display for Cardinality {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> std::fmt::Result { write!( f, "{:?}", self )}
}
