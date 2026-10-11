//! The loot filter window: what auto-loot and autofarm may pick up, by rarity
//! and by item type. Manual pickups are never filtered.

use crate::helper::window_chrome::{draw_sys_button, draw_titlebar, label_color, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::automation::{ItemKind, LootFilter, LootKinds, QolOptions, Rarity};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const LOOT_FILTER_WINDOW_ID: WidgetId = WidgetId(6300);
const CLOSE_BTN_ID: WidgetId = WidgetId(6301);
const ALL_BTN_ID: WidgetId = WidgetId(6302);
const NONE_BTN_ID: WidgetId = WidgetId(6303);
/// Rarity buttons take five consecutive ids from here.
const RARITY_BTN_BASE: u32 = 6310;
/// Item type checkboxes take eight consecutive ids from here.
const KIND_CB_BASE: u32 = 6320;

const WIN_W: f32 = 340.0;
const TITLE_H: f32 = 20.0;
const CLOSE_SIZE: f32 = 11.0;
const ROW_H: f32 = 22.0;
const PAD: f32 = 8.0;
const CB_SIZE: f32 = 11.0;
const KIND_ROWS: usize = 4;

const ROW_RARITY_LABEL: usize = 0;
const ROW_RARITY_BUTTONS: usize = 1;
const ROW_RARITY_CAPTION: usize = 2;
const ROW_KIND_LABEL: usize = 3;
const ROW_KINDS: usize = 4;
const ROW_NOTE: usize = ROW_KINDS + KIND_ROWS;
const ROW_COUNT: usize = ROW_NOTE + 2;
const WIN_H: f32 = TITLE_H + PAD + ROW_COUNT as f32 * ROW_H + PAD;

const SELECTED_TINT: [f32; 4] = [0.2, 0.45, 0.9, 0.4];

pub struct LootFilterWindow {
    pub open: bool,
    /// The current settings; the client copies them in before each build.
    pub qol: QolOptions,
    /// Whether the reference server can be asked about items. Without it the
    /// filter has nothing to judge by.
    pub reference_available: bool,
}

impl Default for LootFilterWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// The colour of a rarity's marker under its button.
fn rarity_color(rarity: Rarity) -> [f32; 4] {
    let [r, g, b] = match rarity {
        Rarity::Common => [0.55, 0.55, 0.55],
        Rarity::Uncommon => [0.25, 0.7, 0.3],
        Rarity::Legendary => [1.0, 0.75, 0.15],
        other => other.glow_color(0.0).unwrap_or([0.55, 0.55, 0.55]),
    };
    [r, g, b, 1.0]
}

/// What a minimum rarity lets through, in words.
fn rarity_caption(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "Everything is picked up.",
        Rarity::Uncommon => "Items dropped at under 10%, and rarer.",
        Rarity::Rare => "Items dropped at under 2% (and MVP drops), and rarer.",
        Rarity::Epic => "Items dropped at under 0.5%, and rarer.",
        Rarity::Legendary => "Items dropped at 0.1% or less.",
    }
}

impl LootFilterWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            qol: QolOptions::default(),
            reference_available: false,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    fn event(&self, filter: LootFilter) -> GameEvent {
        let mut options = self.qol;
        options.loot_filter = filter;
        GameEvent::QolSettingsChanged { options }
    }
}

impl Window for LootFilterWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for LootFilterWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let default_x = ((ui.ctx.screen_width - WIN_W) / 2.0).floor();
        let default_y = ((ui.ctx.screen_height - WIN_H) / 2.0).floor();
        let win = ui.window_at(LOOT_FILTER_WINDOW_ID, WIN_W, WIN_H, TITLE_H, default_x, default_y);
        ui.interact(LOOT_FILTER_WINDOW_ID, Rect::new(win.x, win.y, WIN_W, WIN_H));

        crate::helper::fallback::window_body(ui, win.x, win.y + TITLE_H, WIN_W, WIN_H - TITLE_H);
        draw_titlebar(ui, win.x, win.y, WIN_W, TITLE_H);
        ui.text(win.x + 20.0, win.y + TITLE_H - 5.0, "Loot Filter", text_color());

        let close_rect = Rect::new(
            win.x + WIN_W - CLOSE_SIZE - 4.0,
            win.y + 4.0,
            CLOSE_SIZE,
            CLOSE_SIZE,
        );
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(ui, close_rect, (CLOSE_SIZE, CLOSE_SIZE), close_resp.hovered(), Some('x'));
        if close_resp.clicked() {
            self.open = false;
            return events;
        }

        let row_y = |n: usize| win.y + TITLE_H + PAD + n as f32 * ROW_H;
        let text_y = |n: usize| row_y(n) + 13.0;
        let x0 = win.x + 10.0;
        let label = text_color();
        let mut filter = self.qol.loot_filter;

        // Minimum rarity.
        ui.text(x0, text_y(ROW_RARITY_LABEL), "Pick up items at least as rare as:", label_color());
        let button_w = (WIN_W - PAD * 2.0) / Rarity::ALL.len() as f32;
        for (n, rarity) in Rarity::ALL.into_iter().enumerate() {
            let rect = Rect::new(
                win.x + PAD + n as f32 * button_w,
                row_y(ROW_RARITY_BUTTONS),
                button_w - 2.0,
                ROW_H - 2.0,
            );
            let resp = ui.button(WidgetId(RARITY_BTN_BASE + n as u32), rect, rarity.label());
            if filter.min_rarity == rarity {
                let (v, i) = draw::rounded_rect(rect.x, rect.y, rect.w, rect.h, 3.0, SELECTED_TINT);
                ui.draw_calls.push(DrawCall {
                    vertices: v,
                    indices: i,
                    texture: TextureRef::White,
                });
                let w = ui.atlas.measure_text(rarity.label());
                ui.text(rect.x + (rect.w - w) / 2.0, rect.y + rect.h - 6.0, rarity.label(), label);
            }
            let (v, i) = draw::rounded_rect(rect.x + 4.0, rect.y + rect.h - 3.0, rect.w - 8.0, 2.0, 1.0, rarity_color(rarity));
            ui.draw_calls.push(DrawCall {
                vertices: v,
                indices: i,
                texture: TextureRef::White,
            });
            if resp.clicked() {
                filter.min_rarity = rarity;
            }
        }
        ui.text(x0, text_y(ROW_RARITY_CAPTION), rarity_caption(filter.min_rarity), label_color());

        // Item types.
        ui.text(x0, text_y(ROW_KIND_LABEL), "Pick up these kinds of item:", label_color());
        let small_w = 34.0;
        let none = Rect::new(win.x + WIN_W - PAD - small_w, row_y(ROW_KIND_LABEL), small_w, ROW_H - 4.0);
        let all = Rect::new(none.x - small_w - 4.0, none.y, small_w, none.h);
        if ui.button(ALL_BTN_ID, all, "All").clicked() {
            filter.kinds = LootKinds::all(true);
        }
        if ui.button(NONE_BTN_ID, none, "None").clicked() {
            filter.kinds = LootKinds::all(false);
        }
        let column_w = (WIN_W - PAD * 2.0) / 2.0;
        for (n, kind) in ItemKind::ALL.into_iter().enumerate() {
            let row = ROW_KINDS + n % KIND_ROWS;
            let x = x0 + (n / KIND_ROWS) as f32 * column_w;
            let rect = Rect::new(x, row_y(row) + (ROW_H - CB_SIZE) / 2.0 - 2.0, CB_SIZE, CB_SIZE);
            let mut on = filter.kinds.get(kind);
            if ui
                .checkbox(WidgetId(KIND_CB_BASE + n as u32), rect, &mut on)
                .clicked()
            {
                *filter.kinds.slot_mut(kind) = on;
            }
            ui.text(rect.x + CB_SIZE + 4.0, text_y(row), kind.label(), label);
        }

        // What the filter relies on.
        let (note, color) = if self.reference_available {
            (
                "Items the server can't identify yet are picked up.",
                label_color(),
            )
        } else {
            (
                "No reference server set: nothing can be judged, so everything is picked up.",
                [0.75, 0.1, 0.1, 1.0],
            )
        };
        ui.text(x0, text_y(ROW_NOTE), note, color);
        ui.text(x0, text_y(ROW_NOTE + 1), "Applies to auto-loot and autofarm only.", label_color());

        if filter != self.qol.loot_filter {
            events.push(self.event(filter));
            self.qol.loot_filter = filter;
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    fn click(win: &mut LootFilterWindow, x: f32, y: f32) -> Vec<GameEvent> {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = x;
        ctx.mouse_y = y;
        ctx.mouse_clicked = true;
        let mut state = StateCache::new();
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut character = Character::new();
        let data = DataTable::new();
        win.build(&mut ui, &mut BuildCtx::test(&mut character, &data))
    }

    fn origin() -> (f32, f32) {
        ((800.0 - WIN_W) / 2.0, (600.0 - WIN_H) / 2.0)
    }

    fn row_y(n: usize) -> f32 {
        origin().1 + TITLE_H + PAD + n as f32 * ROW_H
    }

    fn open_window() -> LootFilterWindow {
        let mut win = LootFilterWindow::new();
        win.toggle();
        win
    }

    fn filter_of(events: &[GameEvent]) -> LootFilter {
        match events {
            [GameEvent::QolSettingsChanged { options }] => options.loot_filter,
            other => panic!("unexpected events: {other:?}"),
        }
    }

    fn click_rarity(win: &mut LootFilterWindow, n: usize) -> Vec<GameEvent> {
        let button_w = (WIN_W - PAD * 2.0) / Rarity::ALL.len() as f32;
        click(
            win,
            origin().0 + PAD + n as f32 * button_w + button_w / 2.0 - 1.0,
            row_y(ROW_RARITY_BUTTONS) + 8.0,
        )
    }

    fn click_kind(win: &mut LootFilterWindow, n: usize) -> Vec<GameEvent> {
        let row = ROW_KINDS + n % KIND_ROWS;
        let column_w = (WIN_W - PAD * 2.0) / 2.0;
        click(
            win,
            origin().0 + 10.0 + (n / KIND_ROWS) as f32 * column_w + CB_SIZE / 2.0,
            row_y(row) + (ROW_H - CB_SIZE) / 2.0 - 2.0 + CB_SIZE / 2.0,
        )
    }

    #[test]
    fn it_draws_nothing_while_closed() {
        let mut win = LootFilterWindow::new();
        assert!(click(&mut win, 400.0, 300.0).is_empty());
    }

    #[test]
    fn each_rarity_button_sets_the_minimum() {
        for (n, rarity) in Rarity::ALL.into_iter().enumerate() {
            let mut win = open_window();
            win.qol.loot_filter.min_rarity = if rarity == Rarity::Legendary {
                Rarity::Common
            } else {
                Rarity::Legendary
            };
            let filter = filter_of(&click_rarity(&mut win, n));
            assert_eq!(filter.min_rarity, rarity);
        }
    }

    #[test]
    fn picking_the_current_rarity_again_changes_nothing() {
        let mut win = open_window();
        assert!(click_rarity(&mut win, 0).is_empty(), "common is the default");
    }

    #[test]
    fn each_kind_checkbox_flips_only_its_kind() {
        for (n, kind) in ItemKind::ALL.into_iter().enumerate() {
            let mut win = open_window();
            let filter = filter_of(&click_kind(&mut win, n));
            assert!(!filter.kinds.get(kind), "{kind:?} is switched off");
            for other in ItemKind::ALL.into_iter().filter(|&k| k != kind) {
                assert!(filter.kinds.get(other), "{other:?} stays on");
            }
            assert_eq!(filter.min_rarity, Rarity::Common);
        }
    }

    #[test]
    fn all_and_none_set_every_kind() {
        let small_w = 34.0;
        let none_x = origin().0 + WIN_W - PAD - small_w / 2.0;
        let all_x = none_x - small_w - 4.0;
        let y = row_y(ROW_KIND_LABEL) + 6.0;

        let mut win = open_window();
        let filter = filter_of(&click(&mut win, none_x, y));
        assert_eq!(filter.kinds, LootKinds::all(false));

        let mut win = open_window();
        win.qol.loot_filter.kinds = LootKinds::all(false);
        let filter = filter_of(&click(&mut win, all_x, y));
        assert_eq!(filter.kinds, LootKinds::all(true));
    }

    #[test]
    fn a_change_carries_the_other_settings_along() {
        let mut win = open_window();
        win.qol.auto_loot = true;
        win.qol.farm.heavy_percent = 70;
        match click_rarity(&mut win, 2).as_slice() {
            [GameEvent::QolSettingsChanged { options }] => {
                assert!(options.auto_loot);
                assert_eq!(options.farm.heavy_percent, 70);
                assert_eq!(options.loot_filter.min_rarity, Rarity::Rare);
            }
            other => panic!("unexpected events: {other:?}"),
        }
    }

    #[test]
    fn captions_name_the_drop_rates() {
        assert!(rarity_caption(Rarity::Uncommon).contains("10%"));
        assert!(rarity_caption(Rarity::Rare).contains("2%"));
        assert_eq!(rarity_caption(Rarity::Common), "Everything is picked up.");
    }
}
