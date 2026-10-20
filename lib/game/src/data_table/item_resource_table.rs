use std::collections::HashMap;

use ragnarok_formats::grf::GrfArchive;
use ragnarok_formats::lua_table;

use crate::plugin_catalog::is_plugin_item;

pub struct ItemResourceTable {
    identified_entries: HashMap<u16, String>,
    unidentified_entries: HashMap<u16, String>,
    /// Floor sprite names of plugin items, taken from their `look` item.
    plugin_sprites: HashMap<u16, String>,
}

const IDENTIFIED_PATH: &str = ragnarok_resources::table::IDENTIFIED_ITEM_RESOURCE;
const UNIDENTIFIED_PATH: &str = ragnarok_resources::table::UNIDENTIFIED_ITEM_RESOURCE;

/// The icon file for a resource name from this table: a base-game name, or the
/// full path of a plugin icon.
pub fn resource_icon_path(resource: &str) -> String {
    if resource.starts_with(ragnarok_resources::dir::DATA) {
        resource.to_string()
    } else {
        ragnarok_resources::ui::item::icon(resource)
    }
}

impl ItemResourceTable {
    pub fn from_entries(
        identified_entries: HashMap<u16, String>,
        unidentified_entries: HashMap<u16, String>,
    ) -> Self {
        Self {
            identified_entries,
            unidentified_entries,
            plugin_sprites: HashMap::new(),
        }
    }

    /// Adds a plugin item. Its icon is `icon`, a path under `data/`, when set;
    /// otherwise it is the icon of its `look` item. Its floor sprite is the
    /// sprite of its `look` item.
    pub fn insert_plugin(&mut self, item_id: u16, look: Option<u16>, icon: Option<&str>) {
        let look_name = look
            .and_then(|id| self.get_resource_name(id))
            .map(str::to_string);
        match icon {
            Some(path) => {
                let name = format!("{}{path}", ragnarok_resources::dir::DATA);
                self.identified_entries.insert(item_id, name.clone());
                self.unidentified_entries.insert(item_id, name);
            }
            None => match &look_name {
                Some(name) => {
                    self.identified_entries.insert(item_id, name.clone());
                    self.unidentified_entries.insert(item_id, name.clone());
                }
                None => {
                    self.identified_entries.remove(&item_id);
                    self.unidentified_entries.remove(&item_id);
                }
            },
        }
        match look_name {
            Some(name) => self.plugin_sprites.insert(item_id, name),
            None => self.plugin_sprites.remove(&item_id),
        };
    }

    pub fn load(grf: &GrfArchive) -> Self {
        let identified_entries = grf
            .read_file(IDENTIFIED_PATH)
            .map(|data| lua_table::parse_item_res_table(&data))
            .unwrap_or_default();
        let unidentified_entries = grf
            .read_file(UNIDENTIFIED_PATH)
            .map(|data| lua_table::parse_item_res_table(&data))
            .unwrap_or_default();

        tracing::info!(
            "Loaded item resource tables from GRF: {} identified, {} unidentified",
            identified_entries.len(),
            unidentified_entries.len(),
        );
        if unidentified_entries.is_empty() {
            tracing::warn!(
                "{UNIDENTIFIED_PATH} is missing: unidentified items will show their real icon"
            );
        }

        Self {
            identified_entries,
            unidentified_entries,
            plugin_sprites: HashMap::new(),
        }
    }

    pub fn get_resource_name(&self, item_id: u16) -> Option<&str> {
        self.identified_entries.get(&item_id).map(|s| s.as_str())
    }

    pub fn get_resource_name_for(&self, item_id: u16, is_identified: bool) -> Option<&str> {
        if is_identified {
            self.identified_entries.get(&item_id).map(|s| s.as_str())
        } else {
            self.unidentified_entries
                .get(&item_id)
                .or_else(|| self.identified_entries.get(&item_id))
                .map(|s| s.as_str())
        }
    }

    /// The name the floor sprite is read from. A plugin item has one only when
    /// its `look` item does.
    pub fn get_sprite_name_for(&self, item_id: u16, is_identified: bool) -> Option<&str> {
        if is_plugin_item(item_id) {
            return self.plugin_sprites.get(&item_id).map(|s| s.as_str());
        }
        self.get_resource_name_for(item_id, is_identified)
    }

    pub fn item_icon_path(&self, item_id: u16) -> Option<String> {
        self.get_resource_name(item_id).map(resource_icon_path)
    }

    pub fn item_sprite_path(&self, item_id: u16, is_identified: bool) -> Option<String> {
        self.get_sprite_name_for(item_id, is_identified)
            .map(|name| ragnarok_resources::sprite::item::of(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table() -> ItemResourceTable {
        let mut identified = HashMap::new();
        identified.insert(501, "빨간포션".to_string());
        identified.insert(1201, "단검".to_string());
        let mut unidentified = HashMap::new();
        unidentified.insert(1201, "무기".to_string());
        ItemResourceTable {
            identified_entries: identified,
            unidentified_entries: unidentified,
            plugin_sprites: HashMap::new(),
        }
    }

    #[test]
    fn get_resource_name_returns_identified() {
        let table = make_table();
        assert_eq!(table.get_resource_name(501), Some("빨간포션"));
        assert_eq!(table.get_resource_name(1201), Some("단검"));
        assert!(table.get_resource_name(999).is_none());
    }

    #[test]
    fn get_resource_name_for_dispatches_by_identified() {
        let table = make_table();
        assert_eq!(table.get_resource_name_for(1201, true), Some("단검"));
        assert_eq!(table.get_resource_name_for(1201, false), Some("무기"));
        assert_eq!(table.get_resource_name_for(501, true), Some("빨간포션"));
        assert_eq!(table.get_resource_name_for(501, false), Some("빨간포션"));
    }

    #[test]
    fn item_icon_path_builds_grf_path() {
        let table = make_table();
        assert_eq!(
            table.item_icon_path(501).unwrap(),
            "data/texture/유저인터페이스/item/빨간포션.bmp"
        );
        assert!(table.item_icon_path(999).is_none());
    }

    #[test]
    fn plugin_item_takes_the_icon_and_sprite_of_its_look_item() {
        let mut table = make_table();
        table.insert_plugin(40000, Some(1201), None);
        assert_eq!(table.get_resource_name(40000), Some("단검"));
        assert_eq!(
            table.item_icon_path(40000).unwrap(),
            "data/texture/유저인터페이스/item/단검.bmp"
        );
        assert_eq!(
            table.item_sprite_path(40000, true).unwrap(),
            ragnarok_resources::sprite::item::of("단검")
        );
    }

    #[test]
    fn plugin_icon_replaces_the_look_icon_but_not_the_sprite() {
        let mut table = make_table();
        table.insert_plugin(40001, Some(501), Some("plugin/demo/potion.png"));
        assert_eq!(
            table.item_icon_path(40001).unwrap(),
            "data/plugin/demo/potion.png"
        );
        assert_eq!(
            table.item_sprite_path(40001, true).unwrap(),
            ragnarok_resources::sprite::item::of("빨간포션")
        );
    }

    #[test]
    fn plugin_item_without_a_look_item_has_no_sprite() {
        let mut table = make_table();
        table.insert_plugin(40002, None, Some("plugin/demo/new.png"));
        assert!(table.item_sprite_path(40002, true).is_none());
        assert_eq!(
            table.item_icon_path(40002).unwrap(),
            "data/plugin/demo/new.png"
        );
    }
}
