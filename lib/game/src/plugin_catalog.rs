//! The items that server plugins add to the base game, as the server's catalog
//! (`data/plugin-catalog.json`) describes them. The server picks their ids from
//! the ranges below, so they never collide with GRF data.

use std::ops::RangeInclusive;

use serde::Deserialize;

/// Where the server serves the catalog, next to the rest of the client data.
pub const CATALOG_PATH: &str = "data/plugin-catalog.json";
pub const PLUGIN_ITEM_IDS: RangeInclusive<u32> = 40000..=59999;

/// Only the items are read so far: the server lists skills and jobs too, but
/// does not send them to clients yet.
#[derive(Debug, Default, Deserialize)]
pub struct PluginCatalog {
    #[serde(default)]
    pub items: Vec<CatalogItem>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogItem {
    pub id: u32,
    /// The plugin that defines the item; `None` once that plugin is gone, when
    /// players keep the item.
    pub plugin: Option<String>,
    pub name: String,
    pub display_name: String,
    /// Lines of text, with `^RRGGBB` colour codes.
    #[serde(default)]
    pub description: Vec<String>,
    #[serde(default)]
    pub slots: u8,
    /// A base-game item whose icon and floor sprite this item is drawn with.
    pub look: Option<u32>,
    /// A PNG path under `data/`, which replaces the icon of `look`.
    pub icon: Option<String>,
}

pub fn is_plugin_item(item_id: u16) -> bool {
    PLUGIN_ITEM_IDS.contains(&u32::from(item_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_item_range_is_recognised() {
        assert!(is_plugin_item(40000));
        assert!(is_plugin_item(59999));
        assert!(!is_plugin_item(501));
        assert!(!is_plugin_item(39999));
    }
}
