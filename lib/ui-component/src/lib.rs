pub mod account;
pub mod game;
pub mod helper;
pub mod widget_id;

rust_i18n::i18n!("locales", fallback = "en");

use ragnarok_ai::config::CompanionAiConfig;
use ragnarok_game::character::Character;
use ragnarok_game::companion::{HomunculusState, MercenaryState};
use ragnarok_game::data_table::DataTable;
use ragnarok_game::event::GameEvent;
use ragnarok_game::friends::FriendList;
use ragnarok_game::guild::Guild;
use ragnarok_game::party::Party;
use ragnarok_game::pet::PetState;
use ragnarok_game::quest::QuestLog;
use ragnarok_ui::frame::UiFrame;

/// Live game state passed to every window's `build`. Windows read the fields
/// they need directly each frame (no snapshot, no per-frame clone). Carries
/// only `ragnarok_game` / `ragnarok_ai` data plus primitives: the crate
/// boundary forbids referencing the client's `GameState`/`World`, so state that
/// lives client-side is applied by the caller before `build`.
pub struct BuildCtx<'a> {
    pub character: &'a mut Character,
    pub data: &'a DataTable,
    pub party: Option<&'a Party>,
    pub friends: &'a FriendList,
    pub guild: Option<&'a Guild>,
    pub quest_log: &'a QuestLog,
    pub homunculus: Option<&'a HomunculusState>,
    pub mercenary: Option<&'a MercenaryState>,
    pub pet: &'a PetState,
    pub companion_ai: &'a mut CompanionAiConfig,
    /// Live job of the player entity — the server only ever announces a job
    /// change as a base-look sprite change, so the entity is the only source.
    pub job_class: u16,
    pub local_aid: u32,
    pub local_gid: u32,
}

#[cfg(test)]
impl<'a> BuildCtx<'a> {
    /// Minimal ctx for window unit tests: real `character`/`data`, everything
    /// else defaulted. The defaults are leaked (fine for a test process).
    pub fn test(character: &'a mut Character, data: &'a DataTable) -> Self {
        BuildCtx {
            character,
            data,
            party: None,
            friends: Box::leak(Box::new(FriendList::default())),
            guild: None,
            quest_log: Box::leak(Box::new(QuestLog::default())),
            homunculus: None,
            mercenary: None,
            pet: Box::leak(Box::new(PetState::default())),
            companion_ai: Box::leak(Box::new(CompanionAiConfig::default())),
            job_class: 0,
            local_aid: 0,
            local_gid: 0,
        }
    }
}

pub trait Window {
    /// Windows draw their chrome procedurally, so none of them has GRF chrome to
    /// preload. These hooks stay as no-op defaults until the client stops calling them.
    fn has_grf_textures(&self) -> bool {
        false
    }
    fn set_has_grf_textures(&mut self, _value: bool) {}
    /// Nominal outer size (width, height) in pixels. The default `(0.0, 0.0)`
    /// marks a window that positions itself (bars, dialogs, full-screen
    /// screens); draggable windows override it so the gallery can lay them out
    /// without overlap.
    fn window_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
    fn grf_texture_paths() -> Vec<&'static str>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

pub trait InGameWindow: Window {
    fn setup_modal(&self, _ui: &mut UiFrame) {}
    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent>;
    /// Whether this window claims Escape right now. The caller offers the key to
    /// the front-most claimant only, so a window that is merely visible without
    /// being dismissable (bars, notifications) keeps the `false` default.
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        false
    }
    /// Escape reached this window: close it, or leave an inner mode (a nested
    /// dialog, a sub-page) and stay open.
    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        Vec::new()
    }
    /// Whether this window is currently answering Enter — a modal waiting for an
    /// OK, or a nested input dialog. The caller then blocks the key for every
    /// window that reads it through [`UiFrame::enter_pressed`], so one press
    /// cannot both confirm the modal and open the chat line.
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        false
    }
}

#[cfg(test)]
mod i18n_tests {
    use rust_i18n::t;

    #[test]
    fn shared_labels_resolve_in_english() {
        assert_eq!(t!("common.ok"), "OK");
        assert_eq!(t!("common.cancel"), "Cancel");
    }

    /// Every literal key passed to `t!` in this crate must exist in `en.yml`.
    /// rust-i18n returns the key itself on a miss, so a typo would otherwise
    /// show up on screen as `basic_info.titel`.
    #[test]
    fn every_t_key_exists_in_en() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut keys = Vec::new();
        collect_t_keys(&src, &mut keys);
        assert!(
            keys.iter().any(|k| k == "common.ok"),
            "key scan found no t! keys; the scanner is broken"
        );
        let missing: Vec<&String> = keys
            .iter()
            .filter(|k| crate::_rust_i18n_try_translate("en", k.as_str()).is_none())
            .collect();
        assert!(missing.is_empty(), "keys missing from locales/en.yml: {missing:?}");
    }

    fn collect_t_keys(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                collect_t_keys(&path, out);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read source file");
            let mut rest = text.as_str();
            while let Some(at) = rest.find("t!(\"") {
                // Skip `format!("`, `write!("` and similar: `t` must start a macro name.
                let starts_name = rest[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                let after = &rest[at + "t!(\"".len()..];
                if let Some(end) = after.find('"') {
                    if starts_name {
                        out.push(after[..end].to_string());
                    }
                    rest = &after[end..];
                } else {
                    break;
                }
            }
        }
    }
}
