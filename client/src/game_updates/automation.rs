//! Drives the quality-of-life automation: auto-loot, autofarm and the item
//! lookups behind the floor-item glow and the loot filter. The rules live in
//! `ragnarok_game::automation`; this module reads the game, acts on it, and
//! keeps them paced.

use crate::reference::ReferenceClient;
use crate::{App, input};
use ragnarok_game::app_state::AppState;
use ragnarok_game::automation::{
    AUTOMATION_TICK_SECS, BUTTERFLY_WING, FARM_RADIUS, HEAL_ITEMS, HEAVY_PERCENT,
    ITEM_USE_GAP_SECS, ItemInfo, ItemKind, LOOT_RADIUS, LootCandidate, Rarity, SELF_HEAL_SKILLS,
    SP_ITEMS, Verdict, is_heavy, is_over_percent, is_under_percent,
};
use ragnarok_game::entity::{EntityCategory, EntityState};
use ragnarok_game::keybinding::HotkeyAction;
use ragnarok_network::{build_use_item_packet, build_use_skill_packet};
use serde_json::Value;

/// Margin, as a fraction of the half-screen, a monster must be inside the view
/// by to count as "in view": a monster at the very edge is not worth chasing.
const VIEW_INSET: f32 = -0.05;

impl App {
    /// Starts autofarm, or stops it when it is running.
    pub(crate) fn toggle_autofarm(&mut self) {
        if self.game.automation.farm.active {
            self.stop_autofarm("Autofarm stopped.");
            return;
        }
        if !self.config.qol.autofarm_enabled {
            self.windows.chat_window.add_system(
                "Autofarm is turned off: tick \"Enable autofarm\" in the Graphic Settings."
                    .to_string(),
            );
            return;
        }
        if self.game.session.app_state != AppState::InGame
            || self.game.world.entities.player().is_none()
        {
            return;
        }
        self.game.automation.farm.start();
        self.game.automation.next_tick = 0.0;
        self.windows.chat_window.add_system(
            "Autofarm started: attacking enemies in view and picking up their drops. \
             Click in the world or press the button again to stop."
                .to_string(),
        );
    }

    /// Stops autofarm and drops the fight it was in, saying why.
    pub(crate) fn stop_autofarm(&mut self, reason: &str) {
        if !self.game.automation.farm.active {
            return;
        }
        self.game.automation.farm.stop();
        self.stop_attacking();
        self.game.pending_casts.pending_pickup_item_id = None;
        self.windows.chat_window.add_system(reason.to_string());
    }

    /// Sends the request to use the inventory item at `index`; false if the
    /// player cannot (hidden).
    pub(crate) fn send_use_item(&mut self, index: u16) -> bool {
        if self.player_hidden() {
            return false;
        }
        let account_id = self
            .game
            .session
            .login_session
            .as_ref()
            .map(|s| s.account_id)
            .unwrap_or(0);
        self.channel.send_packet(build_use_item_packet(
            index,
            account_id,
            self.active_packetver,
        ));
        true
    }

    pub(crate) fn update_automation(&mut self) {
        self.poll_item_info();
        self.sync_qol_windows();
        if self.game.session.app_state != AppState::InGame {
            self.game.automation.farm.stop();
            return;
        }
        let now = self.start_time.elapsed().as_secs_f32();
        if self.game.automation.last_map != self.game.session.current_map {
            self.game.automation.last_map = self.game.session.current_map.clone();
            self.game.automation.loot.clear();
            self.stop_autofarm("Autofarm stopped: you changed maps.");
        }
        if now < self.game.automation.next_tick {
            return;
        }
        self.game.automation.next_tick = now + AUTOMATION_TICK_SECS;

        let hotkey = self
            .config
            .keybindings
            .get(HotkeyAction::ToggleAutofarm)
            .map(|chord| chord.display())
            .unwrap_or_default();
        if self.windows.qol_toolbar.hotkey != hotkey {
            self.windows.autofarm_window.hotkey = hotkey.clone();
            self.windows.qol_toolbar.hotkey = hotkey;
        }
        self.request_floor_item_info();

        let farm_on = self.game.automation.farm.active;
        let loot_on = self.config.qol.auto_loot || farm_on;
        if !loot_on {
            return;
        }

        if farm_on {
            if self.farm_guard() {
                return;
            }
            self.farm_recover(now);
        }
        let can_loot = self.looting_allowed();
        if !self.automation_player_ready() {
            return;
        }
        // A pickup walk is already under way; it re-evaluates every frame.
        if self.game.pending_casts.pending_pickup_item_id.is_some() {
            return;
        }

        if let Some(target) = self.game.combat.attack_target_id {
            if farm_on && self.game.automation.farm.timed_out(target, now) {
                self.game.automation.farm.give_up(target, now);
                self.stop_attacking();
            }
            return;
        }
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        if player.movement.is_moving() {
            return;
        }
        let (px, py) = player.movement.cell_position();
        let player_cell = (i32::from(px), i32::from(py));

        if can_loot && let Some(id) = self.next_loot(player_cell, now) {
            self.game.automation.loot.note_selected(id, now);
            self.game.pending_casts.pending_pickup_item_id = Some(id);
            self.check_pending_pickup();
            return;
        }

        if farm_on {
            let candidates = self.farm_candidates(player_cell);
            let automation = &mut self.game.automation;
            if let Some(id) = automation.farm.choose(&candidates, &mut automation.rng, now)
                && !self.begin_attack(id, true)
            {
                self.game.automation.farm.give_up(id, now);
            }
        }
    }

    /// Hands the windows the settings and the farm's state. Every frame, so a
    /// click shows at once and no window works from a stale copy.
    fn sync_qol_windows(&mut self) {
        let qol = self.config.qol;
        let active = self.game.automation.farm.active;
        let reference = self.reference_base().is_some();
        let windows = &mut self.windows;
        windows.qol_toolbar.qol = qol;
        windows.qol_toolbar.farm_active = active;
        windows.autofarm_window.qol = qol;
        windows.autofarm_window.farm_active = active;
        windows.loot_filter_window.qol = qol;
        windows.loot_filter_window.reference_available = reference;
        windows.graphic_options.set_qol(qol);
    }

    /// The nearest floor item auto-loot should walk to: close, not given up on,
    /// and let through by the loot filter.
    fn next_loot(&mut self, player: (i32, i32), now: f32) -> Option<u32> {
        let nearby: Vec<(u32, u16, i32, i32)> = self
            .game
            .world
            .floor_items
            .values()
            .filter(|item| !item.is_falling)
            .map(|item| (item.id, item.item_id, i32::from(item.x), i32::from(item.y)))
            .filter(|&(_, _, x, y)| (x - player.0).abs().max((y - player.1).abs()) <= LOOT_RADIUS)
            .collect();
        let filter = self.config.qol.loot_filter;
        let mut wanted = Vec::new();
        for (id, item_id, x, y) in nearby {
            match self.game.automation.judge(&filter, item_id, now) {
                Verdict::Take => wanted.push(LootCandidate { id, x, y }),
                Verdict::Skip => {}
                Verdict::Wait => self.request_item_info(item_id),
            }
        }
        self.game.automation.loot.nearest(player, wanted, now)
    }

    /// Whether looting is on hold for weight. Tells the player once when it
    /// starts, and again after it has lifted and come back.
    fn looting_allowed(&mut self) -> bool {
        let inventory = &self.game.character.inventory;
        let held = self.config.qol.auto_loot_pause_when_heavy
            && is_heavy(inventory.weight, inventory.max_weight);
        if !held {
            self.game.automation.heavy_notified = false;
            return true;
        }
        if !self.game.automation.heavy_notified {
            self.game.automation.heavy_notified = true;
            self.windows.chat_window.add_system(format!(
                "Auto-loot is paused: you are carrying more than {HEAVY_PERCENT}% of your weight limit."
            ));
        }
        false
    }

    /// Stops autofarm when the character is too hurt or too heavy to go on, and
    /// warps home if that is asked for. True when it stopped.
    fn farm_guard(&mut self) -> bool {
        let farm = self.config.qol.farm;
        let character = &self.game.character;
        let (hp, max_hp) = (i64::from(character.hp), i64::from(character.max_hp));
        let (weight, max_weight) = (
            i64::from(character.inventory.weight),
            i64::from(character.inventory.max_weight),
        );
        if max_hp > 0 && hp == 0 {
            self.stop_autofarm("Autofarm stopped: you died.");
            return true;
        }
        let flee = i64::from(farm.flee_hp_percent);
        if farm.flee_when_hurt && is_under_percent(hp, max_hp, flee) {
            self.end_farm(
                &format!("Autofarm stopped: your HP fell below {flee}%."),
                farm.return_when_hurt,
            );
            return true;
        }
        let heavy = i64::from(farm.heavy_percent);
        if farm.stop_when_heavy && is_over_percent(weight, max_weight, heavy) {
            self.end_farm(
                &format!("Autofarm stopped: you are carrying more than {heavy}% of your weight limit."),
                farm.return_when_heavy,
            );
            return true;
        }
        false
    }

    /// Stops autofarm for `reason`, then warps to the save point if asked to.
    fn end_farm(&mut self, reason: &str, return_to_save_point: bool) {
        self.stop_autofarm(reason);
        if !return_to_save_point {
            return;
        }
        let wing = self
            .game
            .character
            .inventory
            .all_items()
            .iter()
            .find(|item| item.item_id == BUTTERFLY_WING && item.count > 0)
            .map(|item| item.index);
        let message = match wing {
            Some(index) if self.send_use_item(index) => {
                "Returning to the save point with a Butterfly Wing."
            }
            Some(_) => return,
            None => "You have no Butterfly Wing to return with.",
        };
        self.windows.chat_window.add_system(message.to_string());
    }

    /// Heals and restores SP the way the farm options ask for.
    fn farm_recover(&mut self, now: f32) {
        let farm = self.config.qol.farm;
        let character = &self.game.character;
        let (hp, max_hp) = (i64::from(character.hp), i64::from(character.max_hp));
        let (sp, max_sp) = (i64::from(character.sp), i64::from(character.max_sp));
        if hp == 0 {
            return;
        }
        let hurt = is_under_percent(hp, max_hp, i64::from(farm.heal_hp_percent));
        if hurt && now >= self.game.automation.next_heal_item_at {
            let healed = (farm.heal_with_skills && self.cast_self_heal())
                || (farm.heal_with_items && self.use_first_item(&HEAL_ITEMS));
            if healed {
                self.game.automation.next_heal_item_at = now + ITEM_USE_GAP_SECS;
            }
        }
        if farm.restore_sp
            && is_under_percent(sp, max_sp, i64::from(farm.restore_sp_percent))
            && now >= self.game.automation.next_sp_item_at
            && self.use_first_item(&SP_ITEMS)
        {
            self.game.automation.next_sp_item_at = now + ITEM_USE_GAP_SECS;
        }
    }

    /// Uses the first of `ids` the character carries.
    fn use_first_item(&mut self, ids: &[u16]) -> bool {
        let inventory = &self.game.character.inventory;
        let index = ids.iter().find_map(|&id| {
            inventory
                .all_items()
                .iter()
                .find(|item| item.item_id == id && item.count > 0)
                .map(|item| item.index)
        });
        index.is_some_and(|index| self.send_use_item(index))
    }

    /// Casts the character's healing skill on themselves, if they have one and
    /// can cast it now.
    fn cast_self_heal(&mut self) -> bool {
        if self.player_hidden() || self.game.casting_blocks_action() {
            return false;
        }
        let Some(target) = self.game.world.entities.player_id() else {
            return false;
        };
        let character = &self.game.character;
        let found = SELF_HEAL_SKILLS.iter().find_map(|&skill| {
            let data = character.skills.get_skill(skill)?;
            (data.level > 0 && i32::from(character.sp) >= i32::from(data.sp_cost))
                .then(|| (skill, data.selected_level.clamp(1, data.level)))
        });
        let Some((skill, level)) = found else {
            return false;
        };
        if self.skill_on_cooldown(skill) {
            return false;
        }
        self.channel.send_packet(build_use_skill_packet(
            skill,
            level,
            target,
            self.active_packetver,
        ));
        true
    }

    /// Whether the player is in a state to walk, swing or pick something up.
    fn automation_player_ready(&self) -> bool {
        let Some(player) = self.game.world.entities.player() else {
            return false;
        };
        !matches!(
            player.state(),
            EntityState::Sitting
                | EntityState::Dead
                | EntityState::Casting
                | EntityState::SkillExec
                | EntityState::Pickup
        ) && !self.game.casting_blocks_action()
            && !self.is_local_player_incapacitated()
            && !self.player_hidden()
            && !self.windows.npc_dialog.dialog.is_open()
            && !self.windows.npc_shop.shop.is_open()
            && self.game.pending_casts.pending_skill.is_none()
    }

    /// Living, ordinary monsters near the player and on screen. Bosses are left
    /// to the player.
    fn farm_candidates(&self, player_cell: (i32, i32)) -> Vec<u32> {
        let Some((renderer, coords, _, _)) = self.screen_dims() else {
            return Vec::new();
        };
        let gat = self.game.session.gat.as_ref();
        self.game
            .world
            .entities
            .iter()
            .filter(|e| {
                e.category() == EntityCategory::Monster
                    && !e.is_boss
                    && e.state() != EntityState::Dead
                    && !e.is_fading()
            })
            .filter(|e| {
                let (cx, cy) = e.movement.cell_position();
                (i32::from(cx) - player_cell.0)
                    .abs()
                    .max((i32::from(cy) - player_cell.1).abs())
                    <= FARM_RADIUS
            })
            .filter(|e| {
                let [wx, wy, wz] = input::cell_world_pos(e.movement.position(), gat, coords);
                renderer.camera.is_world_pos_visible(wx, wy, wz, VIEW_INSET)
            })
            .map(|e| e.id)
            .collect()
    }

    /// The reference server's address, if one is set.
    fn reference_base(&self) -> Option<String> {
        self.config
            .reference_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string)
    }

    /// Whether items have to be looked up: for the glow, or for a filter that
    /// judges by rarity and kind.
    fn item_info_wanted(&self) -> bool {
        self.config.qol.rare_item_glow || !self.config.qol.loot_filter.is_open()
    }

    /// Asks the server what an item is, once, so its glow and the loot filter
    /// can go by it.
    pub(crate) fn request_item_info(&mut self, item_id: u16) {
        if !self.item_info_wanted() {
            return;
        }
        let automation = &self.game.automation;
        if automation.items.contains_key(&item_id) || automation.info_requested.contains_key(&item_id)
        {
            return;
        }
        let now = self.start_time.elapsed().as_secs_f32();
        let Some(base) = self.reference_base() else {
            self.game.automation.note_info_unavailable(item_id);
            return;
        };
        let client = self
            .rarity_client
            .get_or_insert_with(|| ReferenceClient::spawn(&base));
        match client.request(format!("/db/items/{item_id}")) {
            Ok(()) => self.game.automation.note_info_requested(item_id, now),
            // A dead worker only costs the glow and the filter their lookups.
            Err(_) => self.game.automation.note_info_unavailable(item_id),
        }
    }

    /// Looks up the items already on the floor that nothing asked about yet,
    /// such as after the glow or the filter was switched on.
    fn request_floor_item_info(&mut self) {
        if !self.item_info_wanted() {
            return;
        }
        let missing: Vec<u16> = self
            .game
            .world
            .floor_items
            .values()
            .map(|item| item.item_id)
            .filter(|id| {
                !self.game.automation.items.contains_key(id)
                    && !self.game.automation.info_requested.contains_key(id)
            })
            .collect();
        for item_id in missing {
            self.request_item_info(item_id);
        }
    }

    fn poll_item_info(&mut self) {
        let Some(client) = &self.rarity_client else {
            return;
        };
        for (path, result) in client.poll() {
            let Some(item_id) = path
                .strip_prefix("/db/items/")
                .and_then(|id| id.parse::<u16>().ok())
            else {
                continue;
            };
            match result {
                Ok(entry) => {
                    self.game
                        .automation
                        .items
                        .insert(item_id, item_info_from_entry(&entry));
                }
                Err(_) => self.game.automation.note_info_unavailable(item_id),
            }
        }
    }
}

/// Rarity of an item from its `/db/items/<id>` entry: its `dropped_by` list
/// holds each monster's drop `rate` (1/10000) and whether it is an MVP drop.
pub(crate) fn rarity_from_entry(entry: &Value) -> Rarity {
    Rarity::from_drops(
        entry["dropped_by"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|drop| {
                let rate = u32::try_from(drop["rate"].as_u64()?).ok()?;
                Some((rate, drop["mvp"].as_bool().unwrap_or(false)))
            }),
    )
}

/// What an `/db/items/<id>` entry says about the item: its rarity and kind.
pub(crate) fn item_info_from_entry(entry: &Value) -> ItemInfo {
    ItemInfo {
        rarity: rarity_from_entry(entry),
        kind: entry["item_type"].as_str().and_then(ItemKind::from_server_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rarity_comes_from_the_rarest_dropper() {
        let entry = json!({ "dropped_by": [
            { "mob": { "id": 1002 }, "rate": 5000, "mvp": false, "card": false },
            { "mob": { "id": 1002 }, "rate": 8, "mvp": false, "card": true },
        ]});
        assert_eq!(rarity_from_entry(&entry), Rarity::Legendary);
    }

    #[test]
    fn an_item_nobody_drops_is_common() {
        assert_eq!(rarity_from_entry(&json!({ "dropped_by": [] })), Rarity::Common);
        assert_eq!(rarity_from_entry(&json!({})), Rarity::Common);
    }

    #[test]
    fn mvp_drops_are_at_least_rare() {
        let entry = json!({ "dropped_by": [{ "rate": 9000, "mvp": true }] });
        assert_eq!(rarity_from_entry(&entry), Rarity::Rare);
    }

    #[test]
    fn an_entry_gives_rarity_and_kind() {
        let entry = json!({
            "id": 509,
            "item_type": "Healing",
            "dropped_by": [{ "rate": 150, "mvp": false }],
        });
        assert_eq!(
            item_info_from_entry(&entry),
            ItemInfo {
                rarity: Rarity::Rare,
                kind: Some(ItemKind::Healing),
            }
        );
    }

    #[test]
    fn an_entry_without_a_known_type_has_no_kind() {
        assert_eq!(item_info_from_entry(&json!({})).kind, None);
        assert_eq!(item_info_from_entry(&json!({ "item_type": "Mystery" })).kind, None);
        assert_eq!(item_info_from_entry(&json!({ "item_type": 5 })).kind, None);
    }
}
