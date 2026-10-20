//! Reads the server's plugin catalog, which adds plugin-defined items to the
//! item tables. A server without plugin data serves no catalog file, and then
//! nothing is added.

use ragnarok_game::plugin_catalog::PluginCatalog;

/// Parses the catalog served at `data/plugin-catalog.json`. Skills and jobs in
/// it are ignored for now.
pub(crate) fn parse(bytes: &[u8]) -> Result<PluginCatalog, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_reads_the_servers_field_names() {
        let json = br#"{
            "items": [{
                "id": 40000, "plugin": "demo", "name": "DEMO_RING",
                "display_name": "Demo Ring", "description": ["A ring."],
                "item_type": "Armor", "slots": 1, "look": 2215, "icon": null
            }],
            "skills": [],
            "jobs": []
        }"#;
        let catalog = parse(json).unwrap();
        let item = &catalog.items[0];
        assert_eq!(item.id, 40000);
        assert_eq!(item.display_name, "Demo Ring");
        assert_eq!(item.description, ["A ring."]);
        assert_eq!(item.slots, 1);
        assert_eq!(item.look, Some(2215));
        assert_eq!(item.icon, None);
    }

    #[test]
    fn a_catalog_from_a_live_server_parses() {
        // As served by the lucky_charms example plugin.
        let json = br#"{"items":[{"id":40001,"plugin":"lucky_charms","name":"Lucky_Ticket","display_name":"Lucky Ticket","description":["A ticket for the lucky draw.","Use it to draw one of the ^0000FFLucky Charms^000000 rewards."],"item_type":"Usable","slots":0,"look":6153,"icon":"plugin/lucky_charms/ticket.png"}],"skills":[],"jobs":[]}"#;
        let catalog = parse(json).unwrap();
        let item = &catalog.items[0];
        assert_eq!(item.plugin.as_deref(), Some("lucky_charms"));
        assert_eq!(
            item.description[1],
            "Use it to draw one of the ^0000FFLucky Charms^000000 rewards."
        );
        assert_eq!(item.look, Some(6153));
        assert_eq!(item.icon.as_deref(), Some("plugin/lucky_charms/ticket.png"));
    }

    #[test]
    fn a_retired_item_and_an_empty_catalog_still_parse() {
        let json = br#"{
            "items": [{
                "id": 40001, "plugin": null, "name": "OLD", "display_name": "Old",
                "look": null, "icon": "plugin/demo/old.png"
            }]
        }"#;
        let catalog = parse(json).unwrap();
        assert_eq!(catalog.items[0].plugin, None);
        assert_eq!(catalog.items[0].icon.as_deref(), Some("plugin/demo/old.png"));
        assert!(parse(b"{}").unwrap().items.is_empty());
    }
}
