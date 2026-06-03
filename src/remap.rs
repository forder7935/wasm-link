//! Export name remapping for plugin implementations.
//!
//! Remaps are per-plugin lookup tables. They let a plugin satisfy a canonical
//! [`Binding`](crate::Binding) even when the plugin exports matching interfaces or
//! functions under different names.

use std::collections::HashMap;

/// Maps requested item names to the item names exported by a plugin.
///
/// The table direction is always:
///
/// ```text
/// requested item name -> exported item name
/// ```
pub type ItemResolutionTable = HashMap<String, String>;

/// Describes where a requested interface is found in a plugin's exports.
///
/// `Plugin::remap_interfaces` stores these values in a map whose key is the
/// requested interface name from the [`Binding`](crate::Binding). The [`Remap`]
/// value then describes the interface name and item names actually exported by
/// that plugin.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use wasm_link::Remap;
///
/// let interface_only = Remap::found_as( "legacy-root" );
/// let one_item = Remap::resolves_item( "get-value", "legacy-get-value" );
/// let many_items = Remap::item_resolution_table( HashMap::from([
/// 	( "get-value".to_string(), "legacy-get-value".to_string() ),
/// ]));
/// let interface_and_items = Remap::found_as_with_item_resolution_table(
/// 	"legacy-root",
/// 	HashMap::from([( "get-value".to_string(), "legacy-get-value".to_string() )]),
/// );
/// # let _ = ( interface_only, one_item, many_items, interface_and_items );
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Remap {
    interface: Option<String>,
    item_resolution_table: ItemResolutionTable,
}

impl Remap {
    /// Creates a remap where the requested interface is found under another exported name.
    pub fn found_as(interface: impl Into<String>) -> Self {
        Self {
            interface: Some(interface.into()),
            item_resolution_table: HashMap::new(),
        }
    }

    /// Creates a remap where one requested item resolves to another exported item name.
    ///
    /// The first argument is the requested item name. The second argument is the
    /// item name exported by the plugin.
    pub fn resolves_item(
        requested_item: impl Into<String>,
        exported_item: impl Into<String>,
    ) -> Self {
        Self {
            interface: None,
            item_resolution_table: HashMap::from([(requested_item.into(), exported_item.into())]),
        }
    }

    /// Creates a remap from an item resolution table.
    ///
    /// The table direction is `requested item name -> exported item name`.
    pub fn item_resolution_table(item_resolution_table: ItemResolutionTable) -> Self {
        Self {
            interface: None,
            item_resolution_table,
        }
    }

    /// Creates a remap for both the interface name and its item names.
    ///
    /// The item table direction is `requested item name -> exported item name`.
    pub fn found_as_with_item_resolution_table(
        interface: impl Into<String>,
        item_resolution_table: ItemResolutionTable,
    ) -> Self {
        Self {
            interface: Some(interface.into()),
            item_resolution_table,
        }
    }

    pub(crate) fn interface_name<'a>( &'a self, requested_interface: &'a str ) -> &'a str {
        self.interface.as_deref().unwrap_or( requested_interface )
    }

    pub(crate) fn item_name<'a>( &'a self, requested_item: &'a str ) -> &'a str {
        self.item_resolution_table
            .get( requested_item )
            .map_or( requested_item, String::as_str )
    }
}
