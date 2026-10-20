//! Auto-travel between towns: the trip the server plans over warps and warper
//! NPCs, and the rules that walk it. Pure state; the client reads the game,
//! sends the packets and feeds the answers back.

/// Chebyshev distance, in cells, at which the character can talk to an NPC.
pub const TALK_RANGE: i32 = 2;
/// Seconds one hop may take before the trip gives up.
pub const HOP_TIMEOUT_SECS: f32 = 60.0;
/// Dialogue pages a trip presses through before it gives up on an NPC.
pub const MAX_DIALOG_PAGES: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HopKind {
    /// A warp cell: standing on it changes the map.
    Warp,
    /// An NPC that sends the character elsewhere: talked to, then its menu is
    /// answered level by level, each entry of `menu` in turn (category, group,
    /// then place).
    Npc { name: String, menu: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelHop {
    /// Map the hop starts on.
    pub map: String,
    /// The warp cell to stand on, or the NPC's cell.
    pub x: u16,
    pub y: u16,
    /// Map the hop arrives on.
    pub to_map: String,
    pub kind: HopKind,
    /// Conditions the server reports on the way (quest, zeny, item...). The
    /// trip still tries; the NPC refuses when they are not met.
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TravelAction {
    /// Walk toward the cell, stopping `range` cells short of it.
    Walk { x: u16, y: u16, range: i32 },
    /// Talk to the NPC of the current hop, which is in range.
    Talk,
    /// Nothing to do this tick: a warp is about to happen, the NPC is busy, or
    /// the way is being planned again.
    Wait,
    /// The character stands on a map the route did not expect (a random warp, or
    /// a landing in another part of a map): plan the way again from here, then
    /// `replace_route`.
    Replan,
    /// The trip reached its town.
    Arrived,
    /// The trip stops here, for the reason given.
    Stop(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Travel {
    /// The town the trip ends in.
    pub to: String,
    hops: Vec<TravelHop>,
    index: usize,
    hop_started: f32,
    /// The NPC has been talked to on this hop.
    contacted: bool,
    /// The NPC conversation is open.
    talking: bool,
    /// The menu entry was chosen; the warp is on its way.
    chosen: bool,
    pages: u32,
    npc: Option<u32>,
    /// How many entries of the current hop's menu have been chosen.
    menu_step: usize,
    /// The way is being planned again from the character's position.
    planning: bool,
}

impl Travel {
    pub fn new(to: String, hops: Vec<TravelHop>, now: f32) -> Self {
        Self {
            to,
            hops,
            index: 0,
            hop_started: now,
            contacted: false,
            talking: false,
            chosen: false,
            pages: 0,
            npc: None,
            menu_step: 0,
            planning: false,
        }
    }

    pub fn hops(&self) -> &[TravelHop] {
        &self.hops
    }

    /// Whether the way is being planned again, see `TravelAction::Replan`.
    pub fn is_planning(&self) -> bool {
        self.planning
    }

    /// Takes the way planned from where the character stands. An empty way means
    /// it is already in the town.
    pub fn replace_route(&mut self, hops: Vec<TravelHop>, now: f32) {
        self.hops = hops;
        self.index = 0;
        self.planning = false;
        self.reset_hop(now);
    }

    /// The hop being walked, `None` once every hop is done.
    pub fn current(&self) -> Option<&TravelHop> {
        self.hops.get(self.index)
    }

    /// The id of the NPC talked to on the current hop.
    pub fn npc(&self) -> Option<u32> {
        self.npc
    }

    /// Records the id of the NPC the client found on the hop's cell.
    pub fn set_npc(&mut self, npc_id: u32) {
        self.npc = Some(npc_id);
    }

    /// Decides the next thing to do, given the map and cell the character stands on.
    pub fn step(&mut self, now: f32, map: &str, player: (i32, i32)) -> TravelAction {
        if self.planning {
            if now - self.hop_started > HOP_TIMEOUT_SECS {
                return TravelAction::Stop("no way on could be planned from here".into());
            }
            return TravelAction::Wait;
        }
        // Arriving on a hop's map ends that hop; the next one starts from here.
        while let Some(hop) = self.hops.get(self.index) {
            if map != hop.to_map || map == hop.map {
                break;
            }
            self.next_hop(now);
        }
        let Some(hop) = self.hops.get(self.index) else {
            return TravelAction::Arrived;
        };
        if map != hop.map {
            self.planning = true;
            self.hop_started = now;
            return TravelAction::Replan;
        }
        if now - self.hop_started > HOP_TIMEOUT_SECS {
            return TravelAction::Stop(format!("the trip got stuck on {}", hop.map));
        }
        if self.chosen {
            return TravelAction::Wait;
        }
        let (x, y) = (i32::from(hop.x), i32::from(hop.y));
        match &hop.kind {
            HopKind::Warp => {
                if player == (x, y) {
                    TravelAction::Wait
                } else {
                    TravelAction::Walk { x: hop.x, y: hop.y, range: 0 }
                }
            }
            HopKind::Npc { name, .. } => {
                if self.talking {
                    return TravelAction::Wait;
                }
                if self.contacted {
                    return TravelAction::Stop(format!("{name} closed the conversation without sending you on"));
                }
                let in_range = (player.0 - x).abs().max((player.1 - y).abs()) <= TALK_RANGE;
                if !in_range {
                    return TravelAction::Walk { x: hop.x, y: hop.y, range: TALK_RANGE };
                }
                self.contacted = true;
                self.talking = true;
                TravelAction::Talk
            }
        }
    }

    /// The entry of the menu level now shown that takes the trip on, as the
    /// 1-based choice the server wants, or `None` when the level does not offer it.
    pub fn menu_choice(&self, items: &[String]) -> Option<u8> {
        let HopKind::Npc { menu, .. } = &self.current()?.kind else {
            return None;
        };
        let wanted = plain_text(menu.get(self.menu_step)?);
        items
            .iter()
            .position(|item| plain_text(item).eq_ignore_ascii_case(&wanted))
            .map(|index| (index + 1) as u8)
    }

    /// Marks the menu entry that `menu_choice` gave as chosen. The NPC shows the
    /// next menu level after it, or the warp follows when the menu is done.
    pub fn choose(&mut self) {
        self.menu_step += 1;
        if self.menu_step >= self.menu_len() {
            self.chosen = true;
            self.talking = false;
        }
    }

    fn menu_len(&self) -> usize {
        match self.current().map(|hop| &hop.kind) {
            Some(HopKind::Npc { menu, .. }) => menu.len(),
            _ => 0,
        }
    }

    /// Counts a dialogue page; false once the NPC has used up the trip's patience.
    pub fn dialog_next(&mut self) -> bool {
        self.pages += 1;
        self.pages <= MAX_DIALOG_PAGES
    }

    pub fn dialog_closed(&mut self) {
        self.talking = false;
    }

    fn next_hop(&mut self, now: f32) {
        self.index += 1;
        self.reset_hop(now);
    }

    /// Clears what one hop left behind, and starts its clock.
    fn reset_hop(&mut self, now: f32) {
        self.hop_started = now;
        self.contacted = false;
        self.talking = false;
        self.chosen = false;
        self.pages = 0;
        self.npc = None;
        self.menu_step = 0;
    }
}

/// Text without the `^RRGGBB` colour codes the server puts in menu entries.
fn plain_text(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('^') {
        plain.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        match after.get(..6) {
            Some(hex) if hex.bytes().all(|b| b.is_ascii_hexdigit()) => rest = &after[6..],
            _ => {
                plain.push('^');
                rest = after;
            }
        }
    }
    plain.push_str(rest);
    plain.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warp(map: &str, x: u16, y: u16, to_map: &str) -> TravelHop {
        TravelHop { map: map.into(), x, y, to_map: to_map.into(), kind: HopKind::Warp, checks: vec![] }
    }

    fn warper(map: &str, x: u16, y: u16, to_map: &str, menu: &[&str]) -> TravelHop {
        TravelHop {
            map: map.into(),
            x,
            y,
            to_map: to_map.into(),
            kind: HopKind::Npc {
                name: "Warper".into(),
                menu: menu.iter().map(|entry| entry.to_string()).collect(),
            },
            checks: vec![],
        }
    }

    #[test]
    fn a_warp_is_walked_onto_and_the_trip_moves_on_with_the_map() {
        let mut travel = Travel::new("geffen".into(), vec![warp("prontera", 156, 20, "prt_fild08"), warp("prt_fild08", 170, 380, "geffen")], 0.0);
        assert_eq!(travel.step(0.0, "prontera", (150, 20)), TravelAction::Walk { x: 156, y: 20, range: 0 });
        assert_eq!(travel.step(0.5, "prontera", (156, 20)), TravelAction::Wait);
        assert_eq!(travel.step(1.0, "prt_fild08", (120, 60)), TravelAction::Walk { x: 170, y: 380, range: 0 });
        assert_eq!(travel.step(2.0, "geffen", (120, 60)), TravelAction::Arrived);
    }

    #[test]
    fn an_npc_is_walked_to_then_talked_to_once() {
        let mut travel = Travel::new("geffen".into(), vec![warper("prt_fild08", 160, 30, "geffen", &["Geffen"])], 0.0);
        assert_eq!(travel.step(0.0, "prt_fild08", (100, 30)), TravelAction::Walk { x: 160, y: 30, range: TALK_RANGE });
        assert_eq!(travel.step(0.1, "prt_fild08", (159, 30)), TravelAction::Talk);
        assert_eq!(travel.step(0.2, "prt_fild08", (159, 30)), TravelAction::Wait);
    }

    #[test]
    fn a_closed_conversation_without_a_warp_stops_the_trip() {
        let mut travel = Travel::new("geffen".into(), vec![warper("prt_fild08", 160, 30, "geffen", &["Geffen"])], 0.0);
        travel.step(0.0, "prt_fild08", (159, 30));
        travel.dialog_closed();
        assert!(matches!(travel.step(0.1, "prt_fild08", (159, 30)), TravelAction::Stop(_)));
    }

    #[test]
    fn the_menu_entry_is_found_past_colour_codes() {
        let travel = Travel::new("geffen".into(), vec![warper("prt_fild08", 160, 30, "geffen", &["Geffen"])], 0.0);
        let items = vec!["Prontera".to_string(), "^3355FFGeffen^000000".to_string()];
        assert_eq!(travel.menu_choice(&items), Some(2));
        assert_eq!(travel.menu_choice(&["Morocc".to_string()]), None);
    }

    #[test]
    fn a_three_level_menu_is_answered_one_level_at_a_time() {
        let mut travel = Travel::new(
            "pay_dun04".into(),
            vec![warper("prontera", 159, 192, "pay_dun04", &["Dungeons", "Payon Dungeon", "Payon Dungeon 5"])],
            0.0,
        );
        assert_eq!(travel.step(0.0, "prontera", (159, 190)), TravelAction::Talk);

        let categories = vec!["Towns".to_string(), "Dungeons".to_string()];
        assert_eq!(travel.menu_choice(&categories), Some(2));
        travel.choose();
        assert_eq!(travel.step(0.1, "prontera", (159, 190)), TravelAction::Wait);

        let groups = vec!["Payon Dungeon".to_string()];
        assert_eq!(travel.menu_choice(&groups), Some(1));
        travel.choose();
        assert_eq!(travel.step(0.2, "prontera", (159, 190)), TravelAction::Wait);

        let places = vec!["Payon Dungeon 4".to_string(), "Payon Dungeon 5".to_string()];
        assert_eq!(travel.menu_choice(&places), Some(2));
        travel.choose();
        assert_eq!(travel.menu_choice(&places), None);
        assert_eq!(travel.step(0.3, "prontera", (159, 190)), TravelAction::Wait);
    }

    #[test]
    fn a_stuck_hop_times_out() {
        let mut travel = Travel::new("geffen".into(), vec![warp("prontera", 156, 20, "prt_fild08")], 0.0);
        assert!(matches!(travel.step(HOP_TIMEOUT_SECS + 1.0, "prontera", (150, 20)), TravelAction::Stop(_)));
    }

    #[test]
    fn a_landing_the_route_did_not_expect_asks_for_a_new_way() {
        let mut travel = Travel::new("geffen".into(), vec![warp("prontera", 156, 20, "prt_fild08")], 0.0);
        assert_eq!(travel.step(1.0, "morocc", (1, 1)), TravelAction::Replan);
        assert!(travel.is_planning());
        assert_eq!(travel.step(2.0, "morocc", (1, 1)), TravelAction::Wait);

        let new_way = vec![warp("morocc", 50, 50, "geffen")];
        travel.replace_route(new_way, 3.0);
        assert!(!travel.is_planning());
        assert_eq!(travel.step(3.0, "morocc", (1, 1)), TravelAction::Walk { x: 50, y: 50, range: 0 });
    }

    #[test]
    fn a_way_that_is_never_planned_stops_the_trip() {
        let mut travel = Travel::new("geffen".into(), vec![warp("prontera", 156, 20, "prt_fild08")], 0.0);
        travel.step(1.0, "morocc", (1, 1));
        assert!(matches!(travel.step(1.0 + HOP_TIMEOUT_SECS + 1.0, "morocc", (1, 1)), TravelAction::Stop(_)));
    }

    #[test]
    fn a_replanned_way_already_in_the_town_arrives() {
        let mut travel = Travel::new("geffen".into(), vec![warp("prontera", 156, 20, "prt_fild08")], 0.0);
        travel.step(1.0, "geffen", (1, 1));
        travel.replace_route(vec![], 2.0);
        assert_eq!(travel.step(2.0, "geffen", (1, 1)), TravelAction::Arrived);
    }

    #[test]
    fn an_empty_route_has_arrived_already() {
        let mut travel = Travel::new("prontera".into(), vec![], 0.0);
        assert_eq!(travel.step(0.0, "prontera", (1, 1)), TravelAction::Arrived);
    }
}
