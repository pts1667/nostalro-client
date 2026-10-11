//! Quality-of-life automation: auto-loot, item rarity tiers and the autofarm
//! bookkeeping. Everything here is pure state and rules; the client drives it
//! with the clock and acts on the answers.

use crate::skill::SkillEnum;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Carry weight, in percent of the maximum, above which auto-loot pauses.
pub const HEAVY_PERCENT: i64 = 80;

/// How far, in cells, auto-loot reaches for an item.
pub const LOOT_RADIUS: i32 = 6;
/// How far, in cells, autofarm looks for something to attack.
pub const FARM_RADIUS: i32 = 14;

/// Butterfly Wing: warps to the save point. It is how autofarm "returns to town".
pub const BUTTERFLY_WING: u16 = 602;
/// Healing items, weakest first: autofarm drinks the first kind the player carries.
pub const HEAL_ITEMS: [u16; 7] = [501, 502, 503, 504, 545, 546, 547];
/// SP restoring items (Blue Herb, Blue Potion), weakest first.
pub const SP_ITEMS: [u16; 2] = [510, 505];
/// Skills that heal the caster when aimed at themselves, best first.
pub const SELF_HEAL_SKILLS: [SkillEnum; 1] = [SkillEnum::AlHeal];
/// Seconds between two healing (or two SP) item uses, so a potion's effect
/// lands before the next one is judged necessary.
pub const ITEM_USE_GAP_SECS: f32 = 1.0;
/// Seconds to wait for an item's database entry before a filter stops waiting.
const INFO_WAIT_SECS: f32 = 4.0;

/// Pickups tried per item before it is left alone.
const LOOT_MAX_ATTEMPTS: u8 = 3;
/// Seconds before an item that is still there is tried again.
const LOOT_RETRY_SECS: f32 = 3.0;
/// Seconds after the player drops an item during which an item of that kind
/// appearing on the floor is taken to be theirs.
const OWN_DROP_WINDOW_SECS: f32 = 5.0;

/// Seconds autofarm sticks with one target before it gives up on it.
const FARM_TARGET_TIMEOUT_SECS: f32 = 30.0;
/// Seconds a given-up target is left alone.
const FARM_BLACKLIST_SECS: f32 = 20.0;
/// Seconds between autofarm decisions.
pub const AUTOMATION_TICK_SECS: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QolOptions {
    /// Walk to and pick up items lying within [`LOOT_RADIUS`] while idle.
    pub auto_loot: bool,
    /// Hold auto-loot (and autofarm's looting) back while carrying more than
    /// [`HEAVY_PERCENT`] of the weight limit.
    pub auto_loot_pause_when_heavy: bool,
    /// Coloured glow around rare items on the floor.
    pub rare_item_glow: bool,
    /// Whether autofarm is available at all: its toolbar button and hotkey.
    pub autofarm_enabled: bool,
    /// The QoL toolbar shows only its restore button.
    pub toolbar_minimized: bool,
    pub farm: FarmOptions,
    pub loot_filter: LootFilter,
}

impl Default for QolOptions {
    fn default() -> Self {
        Self {
            auto_loot: false,
            auto_loot_pause_when_heavy: true,
            rare_item_glow: true,
            autofarm_enabled: false,
            toolbar_minimized: false,
            farm: FarmOptions::default(),
            loot_filter: LootFilter::default(),
        }
    }
}

/// What autofarm does to stay safe and supplied. Percentages are of the maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FarmOptions {
    /// Stop farming when the carried weight passes `heavy_percent`.
    pub stop_when_heavy: bool,
    pub heavy_percent: u8,
    /// ...and warp to the save point with a Butterfly Wing.
    pub return_when_heavy: bool,
    /// Stop farming when HP falls below `flee_hp_percent`.
    pub flee_when_hurt: bool,
    pub flee_hp_percent: u8,
    /// ...and warp to the save point with a Butterfly Wing.
    pub return_when_hurt: bool,
    /// Drink healing items when HP falls below `heal_hp_percent`.
    pub heal_with_items: bool,
    /// Heal with the character's own healing skill below `heal_hp_percent`.
    pub heal_with_skills: bool,
    pub heal_hp_percent: u8,
    /// Use SP items when SP falls below `restore_sp_percent`.
    pub restore_sp: bool,
    pub restore_sp_percent: u8,
}

impl Default for FarmOptions {
    fn default() -> Self {
        Self {
            stop_when_heavy: true,
            heavy_percent: 80,
            return_when_heavy: true,
            flee_when_hurt: true,
            flee_hp_percent: 25,
            return_when_hurt: true,
            heal_with_items: true,
            heal_with_skills: true,
            heal_hp_percent: 50,
            restore_sp: true,
            restore_sp_percent: 25,
        }
    }
}

/// The kinds of item the loot filter tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Healing,
    Usable,
    Etc,
    Weapon,
    Armor,
    Card,
    Ammo,
    Pet,
}

impl ItemKind {
    pub const ALL: [ItemKind; 8] = [
        ItemKind::Healing,
        ItemKind::Usable,
        ItemKind::Etc,
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Card,
        ItemKind::Ammo,
        ItemKind::Pet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Healing => "Healing",
            ItemKind::Usable => "Usable",
            ItemKind::Etc => "Etc",
            ItemKind::Weapon => "Weapons",
            ItemKind::Armor => "Armor",
            ItemKind::Card => "Cards",
            ItemKind::Ammo => "Ammo",
            ItemKind::Pet => "Pet items",
        }
    }

    /// The kind for the `item_type` an item entry of the reference server
    /// carries (`Healing`, `Usable`, `DelayConsume`, ...), `None` if unknown.
    pub fn from_server_type(name: &str) -> Option<Self> {
        let key = name.trim().to_ascii_lowercase().replace('_', "");
        let key = key.strip_prefix("it").unwrap_or(&key);
        Some(match key {
            "healing" => ItemKind::Healing,
            "usable" | "delayconsume" | "cash" => ItemKind::Usable,
            "etc" => ItemKind::Etc,
            "weapon" => ItemKind::Weapon,
            "armor" | "shadowgear" => ItemKind::Armor,
            "card" => ItemKind::Card,
            "ammo" => ItemKind::Ammo,
            "petegg" | "petarmor" => ItemKind::Pet,
            _ => return None,
        })
    }
}

/// Which item kinds the loot filter lets through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LootKinds {
    pub healing: bool,
    pub usable: bool,
    pub etc: bool,
    pub weapon: bool,
    pub armor: bool,
    pub card: bool,
    pub ammo: bool,
    pub pet: bool,
}

impl Default for LootKinds {
    fn default() -> Self {
        Self::all(true)
    }
}

impl LootKinds {
    pub fn all(on: bool) -> Self {
        Self {
            healing: on,
            usable: on,
            etc: on,
            weapon: on,
            armor: on,
            card: on,
            ammo: on,
            pet: on,
        }
    }

    pub fn slot_mut(&mut self, kind: ItemKind) -> &mut bool {
        match kind {
            ItemKind::Healing => &mut self.healing,
            ItemKind::Usable => &mut self.usable,
            ItemKind::Etc => &mut self.etc,
            ItemKind::Weapon => &mut self.weapon,
            ItemKind::Armor => &mut self.armor,
            ItemKind::Card => &mut self.card,
            ItemKind::Ammo => &mut self.ammo,
            ItemKind::Pet => &mut self.pet,
        }
    }

    pub fn get(&self, kind: ItemKind) -> bool {
        let mut copy = *self;
        *copy.slot_mut(kind)
    }

    pub fn is_all(&self) -> bool {
        *self == Self::all(true)
    }
}

/// What auto-loot and autofarm are allowed to pick up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LootFilter {
    /// Items at least this rare are picked up.
    pub min_rarity: Rarity,
    pub kinds: LootKinds,
}

impl LootFilter {
    /// Whether the filter lets everything through, so no item needs looking up.
    pub fn is_open(&self) -> bool {
        self.min_rarity == Rarity::Common && self.kinds.is_all()
    }

    /// An item of `kind` (if known) and `rarity` passes.
    pub fn allows(&self, rarity: Rarity, kind: Option<ItemKind>) -> bool {
        rarity >= self.min_rarity
            && match kind {
                Some(kind) => self.kinds.get(kind),
                None => true,
            }
    }
}

/// What the loot filter makes of one floor item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Take,
    Skip,
    /// The item's entry has to arrive before the filter can say.
    Wait,
}

/// Whether `value` is strictly over `percent` of `max`; an unknown `max` is not.
pub fn is_over_percent(value: i64, max: i64, percent: i64) -> bool {
    max > 0 && value * 100 > max * percent
}

/// Whether `value` is strictly under `percent` of `max`; an unknown `max` is not.
pub fn is_under_percent(value: i64, max: i64, percent: i64) -> bool {
    max > 0 && value * 100 < max * percent
}

/// Whether the carried weight is over [`HEAVY_PERCENT`] of the limit. An unknown
/// limit (not yet reported) counts as not heavy.
pub fn is_heavy(weight: i32, max_weight: i32) -> bool {
    is_over_percent(i64::from(weight), i64::from(max_weight), HEAVY_PERCENT)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }

    /// Tier of an item from the monsters that drop it, as `(rate, mvp)` pairs with
    /// the rate in 1/10000. The rarest source decides: a drop at 0.1% or less is
    /// legendary, under 0.5% epic, under 2% rare and under 10% uncommon. Anything
    /// an MVP drops is at least rare.
    pub fn from_drops(drops: impl IntoIterator<Item = (u32, bool)>) -> Self {
        let mut tier = Rarity::Common;
        for (rate, mvp) in drops {
            let by_rate = match rate {
                0..=10 => Rarity::Legendary,
                11..=49 => Rarity::Epic,
                50..=199 => Rarity::Rare,
                200..=999 => Rarity::Uncommon,
                _ => Rarity::Common,
            };
            let by_source = if mvp { Rarity::Rare } else { Rarity::Common };
            tier = tier.max(by_rate).max(by_source);
        }
        tier
    }

    /// What to show before the drop table is known: cards are always rare drops.
    pub fn guess(item_id: u16) -> Self {
        if (4001..=4999).contains(&item_id) {
            Rarity::Epic
        } else {
            Rarity::Common
        }
    }

    /// Glow colour (RGB) at `time` seconds, `None` for items that do not glow.
    /// Legendary items cycle through the whole spectrum.
    pub fn glow_color(self, time: f32) -> Option<[f32; 3]> {
        match self {
            Rarity::Common | Rarity::Uncommon => None,
            Rarity::Rare => Some([0.25, 0.55, 1.0]),
            Rarity::Epic => Some([0.75, 0.3, 1.0]),
            Rarity::Legendary => Some(hue_to_rgb(time * 0.35)),
        }
    }
}

/// A fully saturated colour for a hue given as a fraction of the colour wheel.
fn hue_to_rgb(hue: f32) -> [f32; 3] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let ramp = |offset: f32| (((h + offset).rem_euclid(6.0) - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    [ramp(0.0), ramp(4.0), ramp(2.0)]
}

/// How strongly an item's glow shows at `time` seconds, in `0.3..=1.0`. `seed`
/// (the floor item's id) keeps neighbouring items out of step.
pub fn glow_pulse(time: f32, seed: u32) -> f32 {
    0.65 + 0.35 * (time * 3.0 + seed as f32 * 1.7).sin()
}

/// An item lying on the floor, as far as looting cares.
#[derive(Debug, Clone, Copy)]
pub struct LootCandidate {
    pub id: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default)]
pub struct LootTracker {
    /// Floor item id -> (times chosen, when it was last tried).
    attempts: HashMap<u32, (u8, f32)>,
    /// Items the player dropped, (item id, when).
    own_drops: Vec<(u16, f32)>,
    ignored: HashSet<u32>,
}

impl LootTracker {
    pub fn eligible(&self, id: u32, now: f32) -> bool {
        if self.ignored.contains(&id) {
            return false;
        }
        match self.attempts.get(&id) {
            None => true,
            Some(&(count, last)) => count < LOOT_MAX_ATTEMPTS && now - last >= LOOT_RETRY_SECS,
        }
    }

    pub fn note_selected(&mut self, id: u32, now: f32) {
        let entry = self.attempts.entry(id).or_insert((0, now));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = now;
    }

    /// A pickup request went out for `id`; the retry delay counts from here.
    pub fn note_sent(&mut self, id: u32, now: f32) {
        if let Some(entry) = self.attempts.get_mut(&id) {
            entry.1 = now;
        }
    }

    pub fn note_own_drop(&mut self, item_id: u16, now: f32) {
        self.own_drops.retain(|&(_, at)| now - at < OWN_DROP_WINDOW_SECS);
        self.own_drops.push((item_id, now));
    }

    /// Whether an item that just appeared is one the player dropped; each drop
    /// accounts for one appearance.
    pub fn claim_own_drop(&mut self, item_id: u16, now: f32) -> bool {
        self.own_drops.retain(|&(_, at)| now - at < OWN_DROP_WINDOW_SECS);
        match self.own_drops.iter().position(|&(id, _)| id == item_id) {
            Some(at) => {
                self.own_drops.remove(at);
                true
            }
            None => false,
        }
    }

    pub fn ignore(&mut self, id: u32) {
        self.ignored.insert(id);
    }

    pub fn forget(&mut self, id: u32) {
        self.attempts.remove(&id);
        self.ignored.remove(&id);
    }

    pub fn clear(&mut self) {
        self.attempts.clear();
        self.ignored.clear();
        self.own_drops.clear();
    }

    /// The nearest eligible item within [`LOOT_RADIUS`] of `player`.
    pub fn nearest(
        &self,
        player: (i32, i32),
        items: impl IntoIterator<Item = LootCandidate>,
        now: f32,
    ) -> Option<u32> {
        items
            .into_iter()
            .map(|item| ((item.x - player.0).abs().max((item.y - player.1).abs()), item.id))
            .filter(|&(dist, id)| dist <= LOOT_RADIUS && self.eligible(id, now))
            .min()
            .map(|(_, id)| id)
    }
}

/// Small xorshift generator; autofarm only needs an unpredictable pick.
#[derive(Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `0..bound`; `bound` must be non-zero.
    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

#[derive(Debug, Default)]
pub struct Farm {
    pub active: bool,
    /// The monster being fought and when it was chosen.
    target: Option<(u32, f32)>,
    /// Monsters given up on, until when.
    blacklist: HashMap<u32, f32>,
}

impl Farm {
    pub fn start(&mut self) {
        self.active = true;
        self.target = None;
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.target = None;
        self.blacklist.clear();
    }

    /// Picks one of `candidates` at random, leaving out the ones given up on.
    pub fn choose(&mut self, candidates: &[u32], rng: &mut Rng, now: f32) -> Option<u32> {
        self.blacklist.retain(|_, until| *until > now);
        let usable: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|id| !self.blacklist.contains_key(id))
            .collect();
        if usable.is_empty() {
            return None;
        }
        let id = usable[rng.below(usable.len())];
        self.target = Some((id, now));
        Some(id)
    }

    /// Whether the fight with `id` has gone on so long that it is stuck.
    pub fn timed_out(&self, id: u32, now: f32) -> bool {
        matches!(self.target, Some((target, since)) if target == id && now - since > FARM_TARGET_TIMEOUT_SECS)
    }

    pub fn give_up(&mut self, id: u32, now: f32) {
        self.blacklist.insert(id, now + FARM_BLACKLIST_SECS);
        if matches!(self.target, Some((target, _)) if target == id) {
            self.target = None;
        }
    }
}

/// What the reference server's entry for an item says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemInfo {
    pub rarity: Rarity,
    pub kind: Option<ItemKind>,
}

#[derive(Debug)]
pub struct Automation {
    pub loot: LootTracker,
    pub farm: Farm,
    pub rng: Rng,
    /// What is known of items by id, once their entry has been read.
    pub items: HashMap<u16, ItemInfo>,
    /// Items whose entry has been asked for, and when; negative infinity marks
    /// one that cannot be had (no server, failed lookup).
    pub info_requested: HashMap<u16, f32>,
    /// Whether the player was told that looting is paused for weight.
    pub heavy_notified: bool,
    /// When the next decision is due.
    pub next_tick: f32,
    /// The map the last tick saw, to notice a change.
    pub last_map: Option<String>,
    /// Earliest time of the next healing / SP item use.
    pub next_heal_item_at: f32,
    pub next_sp_item_at: f32,
}

impl Automation {
    pub fn new(seed: u64) -> Self {
        Self {
            loot: LootTracker::default(),
            farm: Farm::default(),
            rng: Rng::new(seed),
            items: HashMap::new(),
            info_requested: HashMap::new(),
            heavy_notified: false,
            next_tick: 0.0,
            last_map: None,
            next_heal_item_at: 0.0,
            next_sp_item_at: 0.0,
        }
    }

    /// Rarity to draw an item with: the known tier, or the guess until it is known.
    pub fn rarity_of(&self, item_id: u16) -> Rarity {
        self.items
            .get(&item_id)
            .map_or_else(|| Rarity::guess(item_id), |info| info.rarity)
    }

    /// Notes that the entry of `item_id` was asked for at `now`.
    pub fn note_info_requested(&mut self, item_id: u16, now: f32) {
        self.info_requested.entry(item_id).or_insert(now);
    }

    /// The entry of `item_id` cannot be had; filters stop waiting for it.
    pub fn note_info_unavailable(&mut self, item_id: u16) {
        self.info_requested.insert(item_id, f32::NEG_INFINITY);
    }

    /// What `filter` makes of the item `item_id` at `now`. An item whose entry
    /// never arrives is taken: without it there is no basis to refuse.
    pub fn judge(&self, filter: &LootFilter, item_id: u16, now: f32) -> Verdict {
        if filter.is_open() {
            return Verdict::Take;
        }
        if let Some(info) = self.items.get(&item_id) {
            return if filter.allows(info.rarity, info.kind) {
                Verdict::Take
            } else {
                Verdict::Skip
            };
        }
        match self.info_requested.get(&item_id) {
            Some(&at) if now - at > INFO_WAIT_SECS => Verdict::Take,
            _ => Verdict::Wait,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, x: i32, y: i32) -> LootCandidate {
        LootCandidate { id, x, y }
    }

    #[test]
    fn heavy_means_strictly_over_eighty_percent() {
        assert!(!is_heavy(800, 1000));
        assert!(is_heavy(801, 1000));
        assert!(!is_heavy(5000, 0), "an unknown limit is not heavy");
    }

    #[test]
    fn rarity_follows_the_rarest_drop() {
        assert_eq!(Rarity::from_drops([]), Rarity::Common);
        assert_eq!(Rarity::from_drops([(5000, false)]), Rarity::Common);
        assert_eq!(Rarity::from_drops([(1000, false)]), Rarity::Common, "10% is not under 10%");
        assert_eq!(Rarity::from_drops([(999, false)]), Rarity::Uncommon);
        assert_eq!(Rarity::from_drops([(200, false)]), Rarity::Uncommon, "2% is not under 2%");
        assert_eq!(Rarity::from_drops([(199, false)]), Rarity::Rare);
        assert_eq!(Rarity::from_drops([(50, false)]), Rarity::Rare);
        assert_eq!(Rarity::from_drops([(49, false), (5000, false)]), Rarity::Epic);
        assert_eq!(Rarity::from_drops([(10, false)]), Rarity::Legendary);
        assert_eq!(Rarity::from_drops([(1, false)]), Rarity::Legendary);
        assert_eq!(Rarity::from_drops([(9000, true)]), Rarity::Rare);
        assert!(Rarity::Legendary > Rarity::Epic);
        assert!(Rarity::Uncommon > Rarity::Common);
        assert!(Rarity::Common.glow_color(0.0).is_none());
        assert!(Rarity::Uncommon.glow_color(0.0).is_none(), "only rare items glow");
        assert_eq!(Rarity::Rare.glow_color(0.0), Rarity::Rare.glow_color(9.0));
    }

    #[test]
    fn legendary_glow_cycles_through_the_spectrum() {
        let at = |t: f32| Rarity::Legendary.glow_color(t).unwrap();
        assert_eq!(at(0.0), [1.0, 0.0, 0.0], "red at the start of the wheel");
        let third = 1.0 / 3.0 / 0.35;
        let green = at(third);
        assert!(green[1] > 0.99 && green[0] < 0.01 && green[2] < 0.01, "{green:?}");
        for step in 0..100 {
            for channel in at(step as f32 * 0.37) {
                assert!((0.0..=1.0).contains(&channel));
            }
        }
    }

    #[test]
    fn pulse_stays_within_its_band() {
        for step in 0..200 {
            let pulse = glow_pulse(step as f32 * 0.05, 7);
            assert!((0.3 - 1e-4..=1.0 + 1e-4).contains(&pulse), "{pulse}");
        }
    }

    #[test]
    fn cards_are_guessed_rare_until_known() {
        let mut auto = Automation::new(1);
        assert_eq!(auto.rarity_of(4001), Rarity::Epic);
        assert_eq!(auto.rarity_of(501), Rarity::Common);
        auto.items.insert(
            4001,
            ItemInfo {
                rarity: Rarity::Common,
                kind: None,
            },
        );
        assert_eq!(auto.rarity_of(4001), Rarity::Common);
    }

    #[test]
    fn nearest_item_wins_within_the_radius() {
        let tracker = LootTracker::default();
        let items = [item(1, 15, 10), item(2, 11, 10), item(3, 30, 30)];
        assert_eq!(tracker.nearest((10, 10), items, 0.0), Some(2));
        assert_eq!(tracker.nearest((10, 10), [item(3, 30, 30)], 0.0), None);
        assert_eq!(tracker.nearest((10, 10), [item(1, 10 + LOOT_RADIUS, 10)], 0.0), Some(1));
        assert_eq!(tracker.nearest((10, 10), [item(1, 11 + LOOT_RADIUS, 10)], 0.0), None);
    }

    #[test]
    fn an_item_is_retried_after_a_delay_and_then_given_up() {
        let mut tracker = LootTracker::default();
        tracker.note_selected(7, 0.0);
        assert!(!tracker.eligible(7, 1.0), "still within the retry delay");
        assert!(tracker.eligible(7, 3.5));
        tracker.note_selected(7, 3.5);
        tracker.note_selected(7, 7.0);
        assert!(!tracker.eligible(7, 100.0), "three tries are all it gets");

        tracker.forget(7);
        assert!(tracker.eligible(7, 100.0));
    }

    #[test]
    fn a_sent_request_restarts_the_retry_delay() {
        let mut tracker = LootTracker::default();
        tracker.note_selected(7, 0.0);
        tracker.note_sent(7, 5.0);
        assert!(!tracker.eligible(7, 6.0));
        assert!(tracker.eligible(7, 8.5));
    }

    #[test]
    fn items_the_player_dropped_are_recognised_once() {
        let mut tracker = LootTracker::default();
        tracker.note_own_drop(501, 10.0);
        assert!(!tracker.claim_own_drop(502, 10.5), "another kind of item");
        assert!(tracker.claim_own_drop(501, 10.5));
        assert!(!tracker.claim_own_drop(501, 10.6), "one drop, one appearance");

        tracker.note_own_drop(501, 10.0);
        assert!(!tracker.claim_own_drop(501, 20.0), "too long ago");
    }

    #[test]
    fn ignored_items_are_never_chosen() {
        let mut tracker = LootTracker::default();
        tracker.ignore(2);
        assert_eq!(tracker.nearest((0, 0), [item(2, 1, 1), item(3, 4, 4)], 0.0), Some(3));
    }

    #[test]
    fn farm_picks_among_candidates_and_skips_given_up_ones() {
        let mut farm = Farm::default();
        let mut rng = Rng::new(42);
        farm.start();
        assert_eq!(farm.choose(&[], &mut rng, 0.0), None);

        let chosen = farm.choose(&[5], &mut rng, 0.0).unwrap();
        assert_eq!(chosen, 5);
        assert!(!farm.timed_out(5, 10.0));
        assert!(farm.timed_out(5, 31.0));

        farm.give_up(5, 31.0);
        assert_eq!(farm.choose(&[5], &mut rng, 32.0), None, "5 is being left alone");
        assert_eq!(farm.choose(&[5, 6], &mut rng, 32.0), Some(6));
        assert_eq!(farm.choose(&[5], &mut rng, 60.0), Some(5), "the blacklist expires");
    }

    #[test]
    fn farm_choice_is_spread_over_the_candidates() {
        let mut farm = Farm::default();
        let mut rng = Rng::new(7);
        let candidates = [1, 2, 3, 4];
        let mut seen = HashSet::new();
        for step in 0..200 {
            seen.insert(farm.choose(&candidates, &mut rng, step as f32).unwrap());
        }
        assert_eq!(seen.len(), candidates.len());
    }

    #[test]
    fn stopping_farm_forgets_its_state() {
        let mut farm = Farm::default();
        farm.start();
        farm.give_up(3, 0.0);
        farm.stop();
        assert!(!farm.active);
        let mut rng = Rng::new(1);
        assert_eq!(farm.choose(&[3], &mut rng, 1.0), Some(3));
    }

    #[test]
    fn options_default_to_looting_off_and_glow_on() {
        let options = QolOptions::default();
        assert!(!options.auto_loot);
        assert!(options.auto_loot_pause_when_heavy);
        assert!(options.rare_item_glow);
        assert!(!options.autofarm_enabled, "autofarm has to be asked for");
        assert!(!options.toolbar_minimized);
        assert!(options.loot_filter.is_open());
    }

    #[test]
    fn farm_defaults_are_the_cautious_ones() {
        let farm = FarmOptions::default();
        assert!(farm.stop_when_heavy && farm.return_when_heavy);
        assert_eq!(farm.heavy_percent, 80);
        assert!(farm.flee_when_hurt && farm.return_when_hurt);
        assert_eq!(farm.flee_hp_percent, 25);
        assert!(farm.heal_with_items && farm.heal_with_skills);
        assert_eq!(farm.heal_hp_percent, 50);
        assert!(farm.restore_sp);
        assert!(farm.flee_hp_percent < farm.heal_hp_percent, "heal before fleeing");
    }

    #[test]
    fn percent_checks_are_strict_and_ignore_an_unknown_maximum() {
        assert!(!is_over_percent(80, 100, 80));
        assert!(is_over_percent(81, 100, 80));
        assert!(!is_over_percent(500, 0, 80));
        assert!(!is_under_percent(25, 100, 25));
        assert!(is_under_percent(24, 100, 25));
        assert!(!is_under_percent(0, 0, 25), "no maximum yet");
    }

    #[test]
    fn server_item_types_map_to_kinds() {
        assert_eq!(ItemKind::from_server_type("Healing"), Some(ItemKind::Healing));
        assert_eq!(ItemKind::from_server_type("DelayConsume"), Some(ItemKind::Usable));
        assert_eq!(ItemKind::from_server_type("DELAY_CONSUME"), Some(ItemKind::Usable));
        assert_eq!(ItemKind::from_server_type("IT_WEAPON"), Some(ItemKind::Weapon));
        assert_eq!(ItemKind::from_server_type("petegg"), Some(ItemKind::Pet));
        assert_eq!(ItemKind::from_server_type("ShadowGear"), Some(ItemKind::Armor));
        assert_eq!(ItemKind::from_server_type("Etc"), Some(ItemKind::Etc));
        assert_eq!(ItemKind::from_server_type("Max"), None);
        assert_eq!(ItemKind::from_server_type(""), None);
    }

    #[test]
    fn every_kind_has_its_own_switch() {
        for kind in ItemKind::ALL {
            let mut kinds = LootKinds::all(true);
            assert!(kinds.get(kind));
            *kinds.slot_mut(kind) = false;
            assert!(!kinds.get(kind));
            assert!(!kinds.is_all());
            for other in ItemKind::ALL.into_iter().filter(|&k| k != kind) {
                assert!(kinds.get(other), "{other:?} is untouched by {kind:?}");
            }
        }
    }

    #[test]
    fn the_filter_needs_both_rarity_and_kind() {
        let mut filter = LootFilter::default();
        assert!(filter.is_open());
        assert!(filter.allows(Rarity::Common, Some(ItemKind::Etc)));

        filter.min_rarity = Rarity::Uncommon;
        assert!(!filter.is_open());
        assert!(!filter.allows(Rarity::Common, Some(ItemKind::Etc)));
        assert!(filter.allows(Rarity::Uncommon, Some(ItemKind::Etc)));
        assert!(filter.allows(Rarity::Legendary, None), "an unknown kind passes");

        filter.kinds.etc = false;
        assert!(!filter.allows(Rarity::Legendary, Some(ItemKind::Etc)));
        assert!(filter.allows(Rarity::Legendary, Some(ItemKind::Card)));
    }

    fn info(rarity: Rarity, kind: ItemKind) -> ItemInfo {
        ItemInfo {
            rarity,
            kind: Some(kind),
        }
    }

    #[test]
    fn an_open_filter_never_waits() {
        let auto = Automation::new(1);
        assert_eq!(auto.judge(&LootFilter::default(), 501, 0.0), Verdict::Take);
    }

    #[test]
    fn the_filter_waits_for_an_unknown_item_and_then_gives_up_waiting() {
        let mut auto = Automation::new(1);
        let filter = LootFilter {
            min_rarity: Rarity::Rare,
            ..LootFilter::default()
        };
        assert_eq!(auto.judge(&filter, 501, 0.0), Verdict::Wait, "not asked for yet");
        auto.note_info_requested(501, 1.0);
        assert_eq!(auto.judge(&filter, 501, 2.0), Verdict::Wait);
        assert_eq!(auto.judge(&filter, 501, 1.0 + INFO_WAIT_SECS + 0.1), Verdict::Take);

        auto.note_info_unavailable(502);
        assert_eq!(auto.judge(&filter, 502, 0.0), Verdict::Take, "no server to ask");
    }

    #[test]
    fn the_filter_judges_a_known_item_by_its_entry() {
        let mut auto = Automation::new(1);
        let filter = LootFilter {
            min_rarity: Rarity::Rare,
            kinds: LootKinds {
                etc: false,
                ..LootKinds::default()
            },
        };
        auto.items.insert(1, info(Rarity::Common, ItemKind::Healing));
        auto.items.insert(2, info(Rarity::Epic, ItemKind::Healing));
        auto.items.insert(3, info(Rarity::Epic, ItemKind::Etc));
        assert_eq!(auto.judge(&filter, 1, 0.0), Verdict::Skip, "too common");
        assert_eq!(auto.judge(&filter, 2, 0.0), Verdict::Take);
        assert_eq!(auto.judge(&filter, 3, 0.0), Verdict::Skip, "etc is switched off");
    }

    #[test]
    fn a_late_entry_still_decides() {
        let mut auto = Automation::new(1);
        let filter = LootFilter {
            min_rarity: Rarity::Rare,
            ..LootFilter::default()
        };
        auto.note_info_requested(7, 0.0);
        assert_eq!(auto.judge(&filter, 7, 100.0), Verdict::Take, "gave up waiting");
        auto.items.insert(7, info(Rarity::Common, ItemKind::Etc));
        assert_eq!(auto.judge(&filter, 7, 100.0), Verdict::Skip);
    }

    #[test]
    fn the_support_item_lists_hold_real_items() {
        assert!(HEAL_ITEMS.contains(&501));
        assert!(SP_ITEMS.contains(&505));
        assert_eq!(BUTTERFLY_WING, 602);
        assert_eq!(SELF_HEAL_SKILLS[0], SkillEnum::AlHeal);
    }
}
