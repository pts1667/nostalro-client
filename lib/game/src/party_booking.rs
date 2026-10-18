//! The party board (party booking): ads of players looking for party members,
//! which every player online can search.
//!
//! The server keeps one ad per character and tells everyone online when an ad
//! goes up, changes or comes down. The place of an ad is a number that only
//! the client gives a name to, from [`PLACES`].

/// Jobs an ad can ask for.
pub const BOOKING_JOBS: usize = 6;
/// An empty slot of [`BookingAd::jobs`].
pub const NO_JOB: i16 = -1;
/// Ads the server answers a search with at most.
pub const RESULTS_PER_PAGE: usize = 10;
/// A search by level finds the ads of players up to this many levels under it.
pub const LEVEL_SPAN: i16 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookingAd {
    /// Numbers the ads in the order they went up.
    pub index: u32,
    /// The character that put the ad up.
    pub leader: String,
    /// When it went up or last changed, in seconds since the epoch.
    pub posted: u32,
    /// The base level of the leader.
    pub level: i16,
    /// An index of [`PLACES`].
    pub place: i16,
    /// [`NO_JOB`] in the slots left empty.
    pub jobs: [i16; BOOKING_JOBS],
}

impl BookingAd {
    /// The jobs the ad asks for, none when anyone is welcome.
    pub fn wanted(&self) -> impl Iterator<Item = i16> + '_ {
        self.jobs.iter().copied().filter(|job| *job >= 0)
    }

    /// Whether a search with these filters would find the ad.
    pub fn matches(&self, filter: &BookingFilter) -> bool {
        if filter.level != 0 && (self.level < filter.level - LEVEL_SPAN || self.level > filter.level) {
            return false;
        }
        match (filter.place, filter.job) {
            (0, NO_JOB) => true,
            (0, job) => self.jobs.contains(&job),
            (place, NO_JOB) => self.place == place,
            _ => false,
        }
    }
}

/// What a search looks for. The server filters by place or by job, never both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookingFilter {
    /// The ads of players up to [`LEVEL_SPAN`] levels under this; 0 for all.
    pub level: i16,
    /// 0 for anywhere.
    pub place: i16,
    pub job: i16,
}

impl Default for BookingFilter {
    fn default() -> Self {
        Self { level: 0, place: 0, job: NO_JOB }
    }
}

/// A call of the party board to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookingRequest {
    /// Puts up the character's ad.
    Register { level: i16, place: i16, jobs: [i16; BOOKING_JOBS] },
    /// Up to [`RESULTS_PER_PAGE`] ads, from the ad numbered `from` on.
    Search { filter: BookingFilter, from: u32 },
    /// Takes the character's ad down.
    Delete,
    /// Changes the jobs the character's ad asks for.
    Update { jobs: [i16; BOOKING_JOBS] },
}

/// What the server tells the party board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookingNotice {
    /// The answer to a [`BookingRequest::Register`]; it fails when the
    /// character already has an ad up.
    Registered { ok: bool },
    /// The answer to a [`BookingRequest::Search`]; `more` when other ads match.
    Results { ads: Vec<BookingAd>, more: bool },
    /// The answer to a [`BookingRequest::Delete`]; it fails when the character
    /// had no ad up.
    Deleted { ok: bool },
    /// Someone put an ad up.
    Inserted(BookingAd),
    /// Someone changed the jobs of their ad.
    Updated { index: u32, jobs: [i16; BOOKING_JOBS] },
    /// Someone took their ad down.
    Removed { index: u32 },
}

/// The jobs an ad can ask for. The server reads one byte a job when an ad goes
/// up, so the jobs numbered from 256 on (Taekwon and its classes) can't be
/// asked for; a transcendent class goes by its first-life one.
pub const WANTED_JOBS: &[(i16, &str)] = &[
    (1, "Swordman"),
    (2, "Mage"),
    (3, "Archer"),
    (4, "Acolyte"),
    (5, "Merchant"),
    (6, "Thief"),
    (7, "Knight"),
    (14, "Crusader"),
    (8, "Priest"),
    (15, "Monk"),
    (9, "Wizard"),
    (16, "Sage"),
    (11, "Hunter"),
    (19, "Bard"),
    (20, "Dancer"),
    (10, "Blacksmith"),
    (18, "Alchemist"),
    (12, "Assassin"),
    (17, "Rogue"),
    (23, "Super Novice"),
    (24, "Gunslinger"),
    (25, "Ninja"),
];

/// The name of job `job` on the board.
pub fn job_label(job: i16) -> String {
    WANTED_JOBS
        .iter()
        .find(|(id, _)| *id == job)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| crate::job_class::job_class_name(job.max(0) as u16))
}

/// The places an ad can name; the index is the number sent to the server.
pub const PLACES: &[&str] = &[
    "Anywhere",
    "Fields",
    "Prontera Culverts",
    "Payon Cave",
    "Orc Dungeon",
    "Ant Hell",
    "Byalan Island",
    "Geffen Dungeon",
    "Pyramids",
    "Sphinx",
    "Sunken Ship",
    "Mjolnir Coal Mine",
    "Clock Tower",
    "Glast Heim",
    "Turtle Island",
    "Toy Factory",
    "Comodo Caves",
    "Amatsu Dungeon",
    "Gonryun Dungeon",
    "Umbala Dungeon",
    "Niflheim",
    "Louyang Dungeon",
    "Magma Dungeon",
    "Ayothaya Dungeon",
    "Einbroch Mine",
    "Juperos",
    "Bio Laboratory",
    "Thanatos Tower",
    "Odin Temple",
    "Abyss Lake",
    "Kiel Hyre's Academy",
    "Rachel Sanctuary",
    "Ice Cave",
    "Thor's Volcano",
    "MVP hunting",
    "Quests",
    "War of Emperium",
];

/// The name of place `place`.
pub fn place_name(place: i16) -> String {
    usize::try_from(place)
        .ok()
        .and_then(|i| PLACES.get(i))
        .map(|name| (*name).to_string())
        .unwrap_or_else(|| format!("Place {place}"))
}

/// `jobs` packed into the slots of an ad: the first [`BOOKING_JOBS`], the rest
/// left empty.
pub fn job_slots(jobs: &[i16]) -> [i16; BOOKING_JOBS] {
    let mut slots = [NO_JOB; BOOKING_JOBS];
    for (slot, job) in slots.iter_mut().zip(jobs) {
        *slot = *job;
    }
    slots
}

/// How long ago something happened, shortly: "now", "5m", "2h", "3d".
pub fn age_label(now: u64, then: u32) -> String {
    let secs = now.saturating_sub(u64::from(then));
    match secs {
        0..60 => "now".to_string(),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ad(level: i16, place: i16, jobs: &[i16]) -> BookingAd {
        BookingAd { index: 1, leader: "Lead".into(), posted: 0, level, place, jobs: job_slots(jobs) }
    }

    #[test]
    fn filters_match_like_the_server() {
        let knight_hunt = ad(50, 4, &[8, 9]);
        assert!(knight_hunt.matches(&BookingFilter::default()));
        assert!(knight_hunt.matches(&BookingFilter { level: 60, ..Default::default() }));
        assert!(!knight_hunt.matches(&BookingFilter { level: 70, ..Default::default() }));
        assert!(!knight_hunt.matches(&BookingFilter { level: 45, ..Default::default() }));
        assert!(knight_hunt.matches(&BookingFilter { job: 8, ..Default::default() }));
        assert!(!knight_hunt.matches(&BookingFilter { job: 7, ..Default::default() }));
        assert!(knight_hunt.matches(&BookingFilter { place: 4, ..Default::default() }));
        assert!(!knight_hunt.matches(&BookingFilter { place: 5, ..Default::default() }));
    }

    #[test]
    fn jobs_and_places_have_names() {
        assert_eq!(ad(1, 0, &[8]).wanted().collect::<Vec<_>>(), vec![8]);
        assert_eq!(job_label(8), "Priest");
        assert_eq!(job_label(4008), "LordKnight");
        assert_eq!(place_name(0), "Anywhere");
        assert_eq!(place_name(2), "Prontera Culverts");
        assert_eq!(place_name(999), "Place 999");
        assert!(WANTED_JOBS.iter().all(|(id, _)| (0..256).contains(id)));
    }

    #[test]
    fn ages_read_shortly() {
        assert_eq!(age_label(1_000, 990), "now");
        assert_eq!(age_label(1_000, 700), "5m");
        assert_eq!(age_label(10_000, 2_000), "2h");
        assert_eq!(age_label(300_000, 0), "3d");
        assert_eq!(age_label(0, 50), "now");
    }
}
