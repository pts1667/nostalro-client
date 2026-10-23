use std::collections::HashMap;

use ragnarok_formats::grf::GrfArchive;
use ragnarok_formats::lua_table::{self, QuestDisplay};

const PATH: &str = ragnarok_resources::table::QUEST_DISPLAY;
const DEFAULT_ICON: &str = "sg_feel";
/// The artwork of quests that have none: a placeholder, so it is left out.
const NO_IMAGE: &str = "que_noimage";

pub struct QuestDisplayTable {
    entries: HashMap<u32, QuestDisplay>,
}

impl QuestDisplayTable {
    pub fn from_entries(entries: HashMap<u32, QuestDisplay>) -> Self {
        Self { entries }
    }

    pub fn load(grf: &GrfArchive) -> Self {
        let mut entries = grf
            .read_file(PATH)
            .map(|data| lua_table::parse_questid2display(&data))
            .unwrap_or_default();
        // Artwork the GRF lacks is forgotten here, so the quest windows leave its space out
        // rather than draw an empty box.
        for quest in entries.values_mut() {
            if !quest.image_name.is_empty() && !grf.file_exists(&texture(&quest.image_name)) {
                quest.image_name.clear();
            }
            if !quest.icon_name.is_empty() && !grf.file_exists(&texture(&quest.icon_name)) {
                quest.icon_name.clear();
            }
        }
        tracing::info!("Loaded quest display table: {} quests", entries.len());
        Self { entries }
    }

    pub fn get(&self, id: u32) -> Option<&QuestDisplay> {
        self.entries.get(&id)
    }

    pub fn title(&self, id: u32) -> String {
        self.entries
            .get(&id)
            .filter(|q| !q.title.is_empty())
            .map(|q| q.title.clone())
            .unwrap_or_else(|| "Unknown Quest".to_string())
    }

    pub fn summary(&self, id: u32) -> String {
        self.entries
            .get(&id)
            .map(|q| q.summary.clone())
            .unwrap_or_default()
    }

    pub fn description(&self, id: u32) -> String {
        self.entries
            .get(&id)
            .map(|q| q.description.clone())
            .unwrap_or_default()
    }

    /// Icon texture path. Every record in our data is SG_FEEL; the newer `ico_*`
    /// names map back to it.
    pub fn icon_texture(&self, id: u32) -> String {
        let name = self
            .entries
            .get(&id)
            .map(|q| q.icon_name.to_ascii_lowercase())
            .filter(|n| !n.is_empty() && !n.starts_with("ico"))
            .unwrap_or_else(|| DEFAULT_ICON.to_string());
        texture(&name)
    }

    /// The quest's artwork, or `None` when it has none to show.
    pub fn image_texture(&self, id: u32) -> Option<String> {
        self.entries
            .get(&id)
            .map(|q| q.image_name.to_ascii_lowercase())
            .filter(|n| !n.is_empty() && n != NO_IMAGE)
            .map(|n| texture(&n))
    }

    /// The icon and artwork of `ids`, for preloading.
    pub fn textures(&self, ids: impl IntoIterator<Item = u32>) -> Vec<String> {
        let mut paths = Vec::new();
        for id in ids {
            paths.push(self.icon_texture(id));
            paths.extend(self.image_texture(id));
        }
        paths.sort();
        paths.dedup();
        paths
    }
}

fn texture(name: &str) -> String {
    ragnarok_resources::ui::item::icon(&name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(image_name: &str) -> QuestDisplayTable {
        let quest = QuestDisplay {
            title: "Job Change to Merchant".into(),
            icon_name: "SG_FEEL".into(),
            image_name: image_name.into(),
            ..QuestDisplay::default()
        };
        QuestDisplayTable::from_entries(HashMap::from([(1009, quest)]))
    }

    #[test]
    fn a_quest_without_artwork_has_no_image() {
        assert_eq!(table("QUE_NOIMAGE").image_texture(1009), None);
        assert_eq!(table("").image_texture(1009), None);
        assert_eq!(table("que_ep13_a").image_texture(404), None);
        assert_eq!(
            table("que_ep13_a").image_texture(1009),
            Some(ragnarok_resources::ui::item::icon("que_ep13_a"))
        );
    }

    #[test]
    fn textures_are_the_icon_and_the_artwork_there_is() {
        assert_eq!(
            table("QUE_NOIMAGE").textures([1009]),
            vec![ragnarok_resources::ui::item::icon("sg_feel")]
        );
        assert_eq!(table("que_ep13_a").textures([1009]).len(), 2);
    }
}
