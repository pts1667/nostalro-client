//! Drives auto-travel: asks the reference server for the route to a town, walks
//! it hop by hop and answers the warper NPCs on the way. The rules live in
//! `ragnarok_game::travel`; this module reads the game and sends the packets.

use crate::App;
use crate::reference::ROUTE_PATH;
use ragnarok_game::app_state::AppState;
use ragnarok_game::entity::EntityCategory;
use ragnarok_game::travel::{HopKind, Travel, TravelAction, TravelHop};
use ragnarok_network::{
    build_contact_npc_packet, build_npc_menu_select_packet, build_npc_next_packet,
};
use ragnarok_ui_component::game::travel_window::TravelTown;
use serde_json::Value;

/// The choice that cancels an NPC menu.
const MENU_CANCEL: u8 = 255;
/// Destinations asked for per page of the reference listing (its most).
const TOWNS_PAGE: usize = 200;

/// The town list the travel window shows, read a page at a time.
#[derive(Default)]
pub(crate) struct TravelTowns {
    /// Whether the whole list has been read.
    pub loaded: bool,
    /// The page in flight, as the reference client names it.
    pub request: Option<String>,
    /// The towns read so far.
    pub listed: Vec<TravelTown>,
    /// The offset of the next page.
    pub next: usize,
}

impl App {
    /// `/travel <map>`: asks the reference server for the way to the town, which
    /// `deliver_route` takes up when it answers.
    pub(crate) fn start_travel(&mut self, to: &str) {
        let to = to.trim().trim_end_matches(".gat").to_ascii_lowercase();
        if to.is_empty() {
            self.windows
                .chat_window
                .add_system("Usage: /travel <map name>, for instance /travel prontera".to_string());
            return;
        }
        if self.game.session.app_state != AppState::InGame || self.game.world.entities.player().is_none() {
            self.windows
                .chat_window
                .add_system("You are not in a map yet.".to_string());
            return;
        }
        if self.game.travel.is_some() {
            self.windows.chat_window.add_system(
                "A trip is already under way. Click in the world to stop it.".to_string(),
            );
            return;
        }
        let reference = self.config.reference_url.as_deref().map(str::trim).unwrap_or("");
        if reference.is_empty() {
            self.windows.chat_window.add_system(
                "Travel needs reference_url in the config to plan the way.".to_string(),
            );
            return;
        }
        if self.request_route(&to) {
            self.windows
                .chat_window
                .add_system(format!("Planning the way to {to}..."));
        } else {
            self.windows
                .chat_window
                .add_system("You are not in a map yet.".to_string());
        }
    }

    /// Asks the reference server for the way from where the character stands to
    /// the town `to`; `deliver_route` takes the answer. False when there is no map.
    fn request_route(&mut self, to: &str) -> bool {
        let Some(map) = self.game.session.current_map.clone() else {
            return false;
        };
        let Some(player) = self.game.world.entities.player() else {
            return false;
        };
        let (x, y) = player.movement.cell_position();
        let path = format!("{ROUTE_PATH}?from={map}&to={to}&x={x}&y={y}");
        self.travel_pending = Some(path.clone());
        self.request_reference(path);
        true
    }

    /// Takes the route the reference server answered for `path`, when it is the
    /// one this game asked for.
    pub(crate) fn deliver_route(&mut self, path: &str, result: Result<Value, String>) {
        if self.travel_pending.as_deref() != Some(path) {
            return;
        }
        self.travel_pending = None;
        let now = self.start_time.elapsed().as_secs_f32();
        let route = result.and_then(|value| route_from_json(&value, now));
        if self.game.travel.as_ref().is_some_and(Travel::is_planning) {
            match route {
                Ok(route) => {
                    if let Some(trip) = self.game.travel.as_mut() {
                        trip.replace_route(route.hops().to_vec(), now);
                    }
                }
                Err(message) => self.stop_travel(&format!("no way on from here: {message}")),
            }
            return;
        }
        let travel = match route {
            Ok(travel) => travel,
            Err(message) => {
                self.windows
                    .chat_window
                    .add_system(format!("Cannot travel: {message}"));
                return;
            }
        };
        if travel.hops().is_empty() {
            self.windows
                .chat_window
                .add_system(format!("You are already in {}.", travel.to));
            return;
        }
        let mut checks: Vec<&str> = travel
            .hops()
            .iter()
            .flat_map(|hop| hop.checks.iter().map(String::as_str))
            .collect();
        checks.sort_unstable();
        checks.dedup();
        let steps = travel.hops().len();
        self.windows.chat_window.add_system(format!(
            "Travelling to {} in {steps} step{}.",
            travel.to,
            if steps == 1 { "" } else { "s" }
        ));
        if !checks.is_empty() {
            self.windows.chat_window.add_system(format!(
                "The way has conditions ({}); the NPCs may refuse.",
                checks.join(", ")
            ));
        }
        self.game.travel = Some(travel);
    }

    /// Walks the trip on, once per frame while the character stands still.
    pub(crate) fn update_travel(&mut self) {
        self.sync_travel_window();
        if self.game.travel.is_none() {
            return;
        }
        if self.game.session.app_state != AppState::InGame {
            self.stop_travel("you left the game.");
            return;
        }
        if self.game.character.max_hp > 0 && self.game.character.hp == 0 {
            self.stop_travel("you died.");
            return;
        }
        let now = self.start_time.elapsed().as_secs_f32();
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        if player.movement.is_moving() {
            return;
        }
        let (px, py) = player.movement.cell_position();
        let Some(map) = self.game.session.current_map.clone() else {
            return;
        };
        let Some(travel) = self.game.travel.as_mut() else {
            return;
        };
        match travel.step(now, &map, (i32::from(px), i32::from(py))) {
            TravelAction::Walk { x, y, range } => {
                if !self.try_move_toward(i32::from(x), i32::from(y), px, py, range) {
                    self.stop_travel("the way is blocked.");
                }
            }
            TravelAction::Talk => self.travel_talk(),
            TravelAction::Wait => {}
            TravelAction::Replan => {
                let to = self
                    .game
                    .travel
                    .as_ref()
                    .map(|trip| trip.to.clone())
                    .unwrap_or_default();
                self.windows
                    .chat_window
                    .add_system("Off the planned way: planning again from here.".to_string());
                if !self.request_route(&to) {
                    self.stop_travel("you are not in a map.");
                }
            }
            TravelAction::Arrived => self.finish_travel(),
            TravelAction::Stop(reason) => self.stop_travel(&reason),
        }
    }

    /// Talks to the NPC of the current hop, which the character stands next to.
    fn travel_talk(&mut self) {
        let Some((x, y)) = self
            .game
            .travel
            .as_ref()
            .and_then(Travel::current)
            .map(|hop| (hop.x, hop.y))
        else {
            return;
        };
        let npc = self.game.world.entities.iter().find(|entity| {
            entity.category() == EntityCategory::Npc && entity.movement.cell_position() == (x, y)
        });
        let Some(npc_id) = npc.map(|entity| entity.id) else {
            self.stop_travel("the NPC is not on screen.");
            return;
        };
        if let Some(travel) = self.game.travel.as_mut() {
            travel.set_npc(npc_id);
        }
        self.channel
            .send_packet(build_contact_npc_packet(npc_id, self.active_packetver));
    }

    /// Answers the trip's warper with the entry its menu level asks for. False when
    /// the menu is not the trip's, so it is shown as usual.
    pub(crate) fn travel_menu(&mut self, npc_id: u32, items: &[String]) -> bool {
        let Some(travel) = self.game.travel.as_mut() else {
            return false;
        };
        if travel.npc() != Some(npc_id) {
            return false;
        }
        let choice = travel.menu_choice(items);
        match choice {
            Some(choice) => {
                travel.choose();
                self.windows.npc_dialog.dialog.close();
                self.channel.send_packet(build_npc_menu_select_packet(
                    npc_id,
                    choice,
                    self.active_packetver,
                ));
            }
            None => {
                self.stop_travel("the NPC does not offer the way on.");
                self.windows.npc_dialog.dialog.close();
                self.channel.send_packet(build_npc_menu_select_packet(
                    npc_id,
                    MENU_CANCEL,
                    self.active_packetver,
                ));
            }
        }
        true
    }

    /// Presses through a page of the trip's NPC. False when the page is not the
    /// trip's, or when the NPC talks on too long and the page is left to the player.
    pub(crate) fn travel_dialog_next(&mut self, npc_id: u32) -> bool {
        let Some(travel) = self.game.travel.as_mut() else {
            return false;
        };
        if travel.npc() != Some(npc_id) {
            return false;
        }
        if !travel.dialog_next() {
            self.stop_travel("the NPC talks for too long.");
            return false;
        }
        self.channel
            .send_packet(build_npc_next_packet(npc_id, self.active_packetver));
        true
    }

    /// Notes that the trip's NPC closed its conversation.
    pub(crate) fn travel_dialog_closed(&mut self, npc_id: u32) {
        if let Some(travel) = self.game.travel.as_mut()
            && travel.npc() == Some(npc_id)
        {
            travel.dialog_closed();
        }
    }

    /// Ends the trip in its town and says so.
    fn finish_travel(&mut self) {
        if let Some(travel) = self.game.travel.take() {
            self.windows
                .chat_window
                .add_system(format!("Arrived in {}.", travel.to));
        }
    }

    /// Ends the trip early and says why.
    pub(crate) fn stop_travel(&mut self, reason: &str) {
        if self.game.travel.take().is_some() {
            self.windows
                .chat_window
                .add_system(format!("Travel stopped: {reason}"));
        }
    }
}

impl App {
    /// The toolbar button: opens or closes the travel window, reading the town
    /// list the first time it opens.
    pub(crate) fn toggle_travel_window(&mut self) {
        self.windows.travel_window.toggle();
        if self.windows.travel_window.open
            && !self.travel_towns.loaded
            && self.travel_towns.request.is_none()
        {
            self.request_town_page();
        }
    }

    /// Hands the travel window and the toolbar whether a trip is under way.
    pub(crate) fn sync_travel_window(&mut self) {
        let travelling = self.game.travel.is_some();
        self.windows.travel_window.travelling = travelling;
        self.windows.qol_toolbar.travelling = travelling;
    }

    fn request_town_page(&mut self) {
        let reference = self.config.reference_url.as_deref().map(str::trim).unwrap_or("");
        if reference.is_empty() {
            self.windows.travel_window.status =
                "Set reference_url in the config to list the towns.".to_string();
            return;
        }
        let path = format!(
            "/db/destinations?category=Towns&offset={}&limit={TOWNS_PAGE}",
            self.travel_towns.next
        );
        self.travel_towns.request = Some(path.clone());
        self.windows.travel_window.status = "Loading the towns...".to_string();
        self.request_reference(path);
    }

    /// Takes a page of the town list that the reference server answered, and asks
    /// for the next one until the list is whole.
    pub(crate) fn deliver_towns(&mut self, path: &str, result: Result<Value, String>) {
        if self.travel_towns.request.as_deref() != Some(path) {
            return;
        }
        self.travel_towns.request = None;
        match result.and_then(|value| towns_page_from_json(&value)) {
            Ok((towns, total)) => {
                self.travel_towns.listed.extend(towns);
                self.travel_towns.next += TOWNS_PAGE;
                if self.travel_towns.next < total {
                    self.request_town_page();
                    return;
                }
                self.travel_towns.loaded = true;
                let towns = std::mem::take(&mut self.travel_towns.listed);
                let window = &mut self.windows.travel_window;
                window.status = if towns.is_empty() {
                    "The server lists no towns.".to_string()
                } else {
                    "Pick a town and click Go. Walking on the map stops the trip.".to_string()
                };
                window.towns = towns;
            }
            Err(message) => {
                self.travel_towns.listed.clear();
                self.travel_towns.next = 0;
                self.windows.travel_window.status = format!("Cannot list the towns: {message}");
            }
        }
    }
}

/// A page of the destinations listing, asked for with `category=Towns`, and the
/// total of that filtered listing.
fn towns_page_from_json(value: &Value) -> Result<(Vec<TravelTown>, usize), String> {
    let total = value
        .get("total")
        .and_then(Value::as_u64)
        .ok_or("the listing has no total")?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or("the listing has no results")?;
    let towns = results
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            // A destination's id is `<map>/<x>,<y>`; the trip ends on the map.
            let map = entry.get("id")?.as_str()?.split('/').next()?;
            Some(TravelTown {
                name: name.to_string(),
                map: map.to_string(),
            })
        })
        .collect();
    Ok((towns, total as usize))
}

/// The trip the reference server planned: `{ "to": "...", "hops": [...] }`.
pub(crate) fn route_from_json(value: &Value, now: f32) -> Result<Travel, String> {
    let to = text(value, "to")?;
    let hops = value
        .get("hops")
        .and_then(Value::as_array)
        .ok_or("the route has no hops")?
        .iter()
        .map(hop_from_json)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Travel::new(to, hops, now))
}

fn hop_from_json(entry: &Value) -> Result<TravelHop, String> {
    let kind = match text(entry, "kind")?.as_str() {
        "warp" => HopKind::Warp,
        "npc" => HopKind::Npc {
            name: text(entry, "npc")?,
            menu: menu(entry)?,
        },
        other => return Err(format!("unknown kind of step: {other}")),
    };
    let checks = entry
        .get("checks")
        .and_then(Value::as_array)
        .map(|checks| {
            checks
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(TravelHop {
        map: text(entry, "map")?,
        x: cell(entry, "x")?,
        y: cell(entry, "y")?,
        to_map: text(entry, "to_map")?,
        kind,
        checks,
        path: waypoints(entry)?,
    })
}

/// The cells the reference server planned for a hop. A hop without them walks
/// straight to its cell.
fn waypoints(entry: &Value) -> Result<Vec<(u16, u16)>, String> {
    let Some(points) = entry.get("path") else {
        return Ok(Vec::new());
    };
    points
        .as_array()
        .ok_or("the route has a path that is not a list")?
        .iter()
        .map(|point| match point.as_array().map(Vec::as_slice) {
            Some([x, y]) => Ok((coordinate(x)?, coordinate(y)?)),
            _ => Err("the route has a path point that is not a cell".to_string()),
        })
        .collect()
}

fn coordinate(value: &Value) -> Result<u16, String> {
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| "the route has a path point that is not a cell".to_string())
}

/// The entries an NPC hop picks, level by level. A hop without them is refused:
/// the trip could never get past its warper.
fn menu(entry: &Value) -> Result<Vec<String>, String> {
    let entries = entry
        .get("menu")
        .and_then(Value::as_array)
        .ok_or("the route has no menu for an NPC")?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()
        .ok_or("the route has a menu entry that is not text")?;
    if entries.is_empty() {
        return Err("the route has an empty menu for an NPC".into());
    }
    Ok(entries)
}

fn text(entry: &Value, key: &str) -> Result<String, String> {
    entry
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("the route has no {key}"))
}

fn cell(entry: &Value, key: &str) -> Result<u16, String> {
    entry
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| format!("the route has no {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_route_reads_warps_and_warper_npcs() {
        let route = json!({
            "to": "geffen",
            "hops": [
                { "kind": "warp", "map": "prontera", "x": 156, "y": 20, "to_map": "prt_fild08", "checks": [] },
                { "kind": "npc", "map": "prt_fild08", "x": 160, "y": 30, "to_map": "geffen", "npc": "Warper#prt", "destination": "Geffen", "menu": ["Cities", "Geffen"], "checks": [] }
            ]
        });
        let travel = route_from_json(&route, 0.0).unwrap();
        assert_eq!(travel.to, "geffen");
        assert_eq!(travel.hops().len(), 2);
        assert!(travel.hops()[1].checks.is_empty());
        assert_eq!(
            travel.hops()[1].kind,
            HopKind::Npc { name: "Warper#prt".into(), menu: vec!["Cities".into(), "Geffen".into()] }
        );
    }

    #[test]
    fn a_hop_reads_the_path_the_server_planned_and_none_when_absent() {
        let route = json!({
            "to": "geffen",
            "hops": [
                { "kind": "warp", "map": "prontera", "x": 156, "y": 20, "to_map": "prt_fild08", "checks": [], "path": [[150, 20], [156, 20]] },
                { "kind": "warp", "map": "prt_fild08", "x": 170, "y": 380, "to_map": "geffen", "checks": [] }
            ]
        });
        let travel = route_from_json(&route, 0.0).unwrap();
        assert_eq!(travel.hops()[0].path, vec![(150, 20), (156, 20)]);
        assert!(travel.hops()[1].path.is_empty());
    }

    #[test]
    fn a_path_point_that_is_not_a_cell_refuses_the_route() {
        let route = json!({
            "to": "geffen",
            "hops": [
                { "kind": "warp", "map": "prontera", "x": 156, "y": 20, "to_map": "prt_fild08", "checks": [], "path": [[150]] }
            ]
        });
        assert!(route_from_json(&route, 0.0).is_err());
    }

    #[test]
    fn the_server_example_with_a_warper_parses() {
        let route = json!({
            "from": "prt_fild08", "to": "pay_dun04", "region": null,
            "hops": [
                { "kind": "warp", "map": "prt_fild08", "x": 170, "y": 378, "x_size": 3, "y_size": 2, "region": 2, "to_map": "prontera", "to_x": 156, "to_y": 26, "to_region": 2, "checks": [] },
                { "kind": "npc", "map": "prontera", "x": 159, "y": 192, "npc": "Warper#prt", "destination": "Payon Dungeon 5", "menu": ["Dungeons", "Payon Dungeon", "Payon Dungeon 5"], "region": 2, "to_map": "pay_dun04", "to_x": 201, "to_y": 204, "to_region": 2, "checks": [] }
            ]
        });
        let travel = route_from_json(&route, 0.0).unwrap();
        assert_eq!(travel.hops().len(), 2);
        assert_eq!(travel.hops()[0].kind, HopKind::Warp);
        assert_eq!(
            travel.hops()[1].kind,
            HopKind::Npc {
                name: "Warper#prt".into(),
                menu: vec!["Dungeons".into(), "Payon Dungeon".into(), "Payon Dungeon 5".into()],
            }
        );
    }

    #[test]
    fn a_listing_page_gives_the_town_and_its_map() {
        let page = json!({
            "total": 2, "offset": 0, "limit": 200,
            "results": [
                { "id": "prontera/156,26", "name": "Prontera", "href": "/db/destinations/prontera/156,26" },
                { "id": "geffen/120,60", "name": "Geffen", "href": "/db/destinations/geffen/120,60" }
            ]
        });
        let (towns, total) = towns_page_from_json(&page).unwrap();
        assert_eq!(total, 2);
        assert_eq!(
            towns,
            vec![
                TravelTown { name: "Prontera".into(), map: "prontera".into() },
                TravelTown { name: "Geffen".into(), map: "geffen".into() },
            ]
        );
    }

    #[test]
    fn a_listing_without_results_is_refused() {
        assert!(towns_page_from_json(&json!({ "total": 0 })).is_err());
    }

    #[test]
    fn an_npc_hop_without_a_menu_refuses_the_route() {
        let route = json!({ "to": "geffen", "hops": [{ "kind": "npc", "map": "a", "x": 1, "y": 1, "to_map": "b", "npc": "Warper" }] });
        assert!(route_from_json(&route, 0.0).is_err());
        let empty = json!({ "to": "geffen", "hops": [{ "kind": "npc", "map": "a", "x": 1, "y": 1, "to_map": "b", "npc": "Warper", "menu": [] }] });
        assert!(route_from_json(&empty, 0.0).is_err());
    }

    #[test]
    fn a_step_of_unknown_kind_refuses_the_route() {
        let route = json!({ "to": "geffen", "hops": [{ "kind": "boat", "map": "a", "x": 1, "y": 1, "to_map": "b" }] });
        assert!(route_from_json(&route, 0.0).is_err());
    }

    #[test]
    fn a_coordinate_out_of_range_refuses_the_route() {
        let route = json!({ "to": "geffen", "hops": [{ "kind": "warp", "map": "a", "x": -1, "y": 1, "to_map": "b" }] });
        assert!(route_from_json(&route, 0.0).is_err());
    }
}
