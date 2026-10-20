use crate::data_table::accessory_table::AccessoryTable;
use crate::data_table::card_illustration_table::CardIllustrationTable;
use crate::data_table::card_name_table::CardNameTable;
use crate::data_table::item_description_table::ItemDescriptionTable;
use crate::data_table::item_name_table::ItemNameTable;
use crate::data_table::item_resource_table::ItemResourceTable;
use crate::data_table::item_slot_count_table::ItemSlotCountTable;
use crate::data_table::map_name_table::MapNameTable;
use crate::data_table::map_position_table::MapPositionTable;
use crate::data_table::msg_string_table::MsgStringTable;
use crate::data_table::name_table::NameTable;
use crate::data_table::quest_display_table::QuestDisplayTable;
use crate::data_table::skill_description_table::SkillDescriptionTable;
use crate::data_table::skill_name_table::SkillNameTable;
use crate::data_table::skill_tree_table::SkillTreeTable;
use crate::data_table::skill_use_level_table::SkillUseLevelTable;
use crate::plugin_catalog::{PluginCatalog, is_plugin_item};

pub mod accessory_table;
pub mod card_illustration_table;
pub mod card_name_table;
pub mod fog_table;
pub mod indoor_table;
pub mod item_description_table;
pub mod item_name_table;
pub mod item_resource_table;
pub mod item_slot_count_table;
pub mod map_cloud_table;
pub mod map_name_table;
pub mod map_position_table;
pub mod msg_string_table;
pub mod name_table;
pub mod quest_display_table;
pub mod skill_description_table;
pub mod skill_name_table;
pub mod skill_tree_table;
pub mod skill_use_level_table;
#[derive(Default)]
pub struct DataTable {
    pub name: Option<NameTable>,
    pub accessory: Option<AccessoryTable>,
    pub item_name: Option<ItemNameTable>,
    pub item_resource: Option<ItemResourceTable>,
    pub item_slot_count: Option<ItemSlotCountTable>,
    pub card_name: Option<CardNameTable>,
    pub card_illustration: Option<CardIllustrationTable>,
    pub item_description: Option<ItemDescriptionTable>,
    pub skill_name: Option<SkillNameTable>,
    pub skill_description: Option<SkillDescriptionTable>,
    pub skill_tree: Option<SkillTreeTable>,
    pub skill_use_level: Option<SkillUseLevelTable>,
    pub pet_talk: Option<ragnarok_formats::pettalk::PetTalkTable>,
    pub quest_display: Option<QuestDisplayTable>,
    pub msg_string: Option<MsgStringTable>,
    pub map_position: Option<MapPositionTable>,
    pub map_name: Option<MapNameTable>,
}

impl DataTable {
    pub fn new() -> Self {
        Self {
            name: None,
            accessory: None,
            item_name: None,
            item_resource: None,
            item_slot_count: None,
            card_name: None,
            card_illustration: None,
            item_description: None,
            skill_name: None,
            skill_description: None,
            skill_tree: None,
            skill_use_level: None,
            pet_talk: None,
            quest_display: None,
            msg_string: None,
            map_position: None,
            map_name: None,
        }
    }

    /// Adds the plugin catalog's items to the item tables, so they resolve like
    /// base-game items: names, descriptions, slot counts and pictures.
    pub fn add_plugin_items(&mut self, catalog: &PluginCatalog) {
        for item in &catalog.items {
            let Some(id) = u16::try_from(item.id)
                .ok()
                .filter(|&id| is_plugin_item(id))
            else {
                continue;
            };
            if let Some(names) = self.item_name.as_mut() {
                names.insert_plugin(id, item.display_name.clone());
            }
            if let Some(descriptions) = self.item_description.as_mut() {
                descriptions.insert_plugin(id, item.description.clone());
            }
            if let Some(slots) = self.item_slot_count.as_mut() {
                slots.insert_plugin(id, item.slots);
            }
            if let Some(resources) = self.item_resource.as_mut() {
                let look = item.look.and_then(|look| u16::try_from(look).ok());
                resources.insert_plugin(id, look, item.icon.as_deref());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::plugin_catalog::CatalogItem;

    fn catalog_item(id: u32, look: Option<u32>, icon: Option<&str>) -> CatalogItem {
        CatalogItem {
            id,
            plugin: Some("demo".to_string()),
            name: "DEMO_RING".to_string(),
            display_name: "Demo Ring".to_string(),
            description: vec!["A ring from a plugin.".to_string()],
            slots: 1,
            look,
            icon: icon.map(str::to_string),
        }
    }

    fn base_tables() -> DataTable {
        let mut table = DataTable::new();
        let mut names = HashMap::new();
        names.insert(501, "Red Potion".to_string());
        table.item_name = Some(ItemNameTable::from_entries(names, HashMap::new()));
        table.item_description = Some(ItemDescriptionTable::from_entries(
            HashMap::new(),
            HashMap::new(),
        ));
        table.item_slot_count = Some(ItemSlotCountTable::from_entries(HashMap::new()));
        let mut resources = HashMap::new();
        resources.insert(1201, "단검".to_string());
        table.item_resource = Some(ItemResourceTable::from_entries(
            resources,
            HashMap::new(),
        ));
        table
    }

    #[test]
    fn plugin_items_join_the_item_tables() {
        let mut table = base_tables();
        let catalog = PluginCatalog {
            items: vec![catalog_item(40000, Some(1201), None)],
        };
        table.add_plugin_items(&catalog);

        let names = table.item_name.as_ref().unwrap();
        assert_eq!(names.get_name_or_id_for(40000, true), "Demo Ring");
        assert_eq!(names.get_name_or_id_for(40000, false), "Demo Ring");
        let descriptions = table.item_description.as_ref().unwrap();
        assert_eq!(
            descriptions.get(40000, true).unwrap(),
            ["A ring from a plugin."]
        );
        let slots = table.item_slot_count.as_ref().unwrap();
        assert_eq!(slots.get_slot_count(40000), 1);
        let resources = table.item_resource.as_ref().unwrap();
        assert_eq!(resources.get_resource_name(40000), Some("단검"));
    }

    #[test]
    fn plugin_items_never_replace_base_game_items() {
        let mut table = base_tables();
        let catalog = PluginCatalog {
            items: vec![catalog_item(501, None, None)],
        };
        table.add_plugin_items(&catalog);
        let names = table.item_name.as_ref().unwrap();
        assert_eq!(names.get_name_or_id_for(501, true), "Red Potion");
    }
}
