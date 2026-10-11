//! A small toolbar in the bottom-right corner that gathers the quality-of-life
//! features: the autofarm switch, its settings and the loot filter. The strip
//! above the buttons is the drag handle; the last button folds it away.

use crate::helper::window_chrome::text_color;
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::automation::QolOptions;
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const QOL_TOOLBAR_WINDOW_ID: WidgetId = WidgetId(6100);
const FARM_BTN_ID: WidgetId = WidgetId(6101);
const SETUP_BTN_ID: WidgetId = WidgetId(6102);
const FILTER_BTN_ID: WidgetId = WidgetId(6103);
const FOLD_BTN_ID: WidgetId = WidgetId(6104);

const GRIP_H: f32 = 6.0;
const BTN_H: f32 = 20.0;
const PAD: f32 = 3.0;
const GAP: f32 = 2.0;
const WIN_H: f32 = GRIP_H + BTN_H + PAD;
const BUTTON_PAD: f32 = 14.0;
const FOLD_W: f32 = 20.0;
const SCREEN_MARGIN: f32 = 8.0;

const ON_TINT: [f32; 4] = [0.15, 0.75, 0.25, 0.55];
const GRIP_COLOR: [f32; 4] = [0.1, 0.2, 0.55, 0.55];

pub struct QolToolbar {
    /// The current settings; the client copies them in before each build.
    pub qol: QolOptions,
    /// Whether autofarm is running.
    pub farm_active: bool,
    /// How the autofarm hotkey reads, for the tooltip; empty when unbound.
    pub hotkey: String,
    /// Where the window stood last frame, to keep its right edge in place when
    /// it grows or shrinks.
    last_rect: Option<Rect>,
}

impl Default for QolToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl QolToolbar {
    pub fn new() -> Self {
        Self {
            qol: QolOptions::default(),
            farm_active: false,
            hotkey: String::new(),
            last_rect: None,
        }
    }

    fn farm_label(&self) -> &'static str {
        if self.farm_active {
            "Autofarm: ON"
        } else {
            "Autofarm: OFF"
        }
    }

    fn farm_tooltip(&self) -> String {
        let mut text = if self.farm_active {
            "Click to stop attacking enemies on its own".to_string()
        } else {
            "Click to attack enemies in view on its own and pick up their drops".to_string()
        };
        if !self.hotkey.is_empty() {
            text.push_str(&format!(" ({})", self.hotkey));
        }
        text
    }

    /// The buttons that show, left to right, as (id, label).
    fn buttons(&self) -> Vec<(WidgetId, &'static str)> {
        if self.qol.toolbar_minimized {
            return Vec::new();
        }
        let mut buttons = Vec::new();
        if self.qol.autofarm_enabled {
            buttons.push((FARM_BTN_ID, self.farm_label()));
            buttons.push((SETUP_BTN_ID, "Setup"));
        }
        buttons.push((FILTER_BTN_ID, "Loot filter"));
        buttons
    }

    fn width(&self, ui: &UiFrame) -> f32 {
        let buttons = self.buttons();
        let mut w = PAD * 2.0 + FOLD_W;
        for (_, label) in &buttons {
            w += ui.atlas.measure_text(label) + BUTTON_PAD + GAP;
        }
        w
    }

    fn fold_events(&self) -> Vec<GameEvent> {
        let mut options = self.qol;
        options.toolbar_minimized = !options.toolbar_minimized;
        vec![GameEvent::QolSettingsChanged { options }]
    }
}

fn draw_tooltip(ui: &mut UiFrame, rect: Rect, text: &str) {
    let pad = 6.0;
    let box_w = ui.atlas.measure_text(text) + pad * 2.0;
    let box_h = ui.atlas.line_height + pad * 2.0;
    let tx = (rect.x + rect.w - box_w).min(ui.ctx.screen_width - box_w).max(2.0);
    let ty = (rect.y - box_h - 4.0).max(2.0);
    let (v, i) = draw::quad_vertices(tx, ty, box_w, box_h, [0.0, 0.0, 0.0, 0.85]);
    ui.tooltip_draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
    let (v, i) = draw::text_vertices(
        text,
        tx + pad,
        ty + pad + ui.atlas.line_height,
        [0.9, 0.9, 0.9, 1.0],
        ui.atlas,
    );
    if !v.is_empty() {
        ui.tooltip_draw_calls.push(DrawCall {
            vertices: v,
            indices: i,
            texture: TextureRef::FontAtlas,
        });
    }
}

fn tint(ui: &mut UiFrame, rect: Rect, color: [f32; 4]) {
    let (v, i) = draw::rounded_rect(rect.x, rect.y, rect.w, rect.h, 3.0, color);
    ui.draw_calls.push(DrawCall {
        vertices: v,
        indices: i,
        texture: TextureRef::White,
    });
}

impl Window for QolToolbar {
    fn window_size(&self) -> (f32, f32) {
        (PAD * 2.0 + FOLD_W, WIN_H)
    }
}

impl InGameWindow for QolToolbar {
    fn build(&mut self, ui: &mut UiFrame, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let width = self.width(ui);
        if let Some(last) = self.last_rect
            && (last.w - width).abs() > 0.5
        {
            ui.set_window_position(QOL_TOOLBAR_WINDOW_ID, last.x + last.w - width, last.y);
        }
        let default_x = (ui.ctx.screen_width - width - SCREEN_MARGIN).floor();
        let default_y = (ui.ctx.screen_height - WIN_H - SCREEN_MARGIN).floor();
        let win = ui.window_at(
            QOL_TOOLBAR_WINDOW_ID,
            width,
            WIN_H,
            GRIP_H,
            default_x,
            default_y,
        );
        self.last_rect = Some(win);
        ui.interact(QOL_TOOLBAR_WINDOW_ID, win);

        let (v, i) = draw::rounded_rect(win.x + 20.0, win.y + 1.0, (width - 40.0).max(8.0), 3.0, 1.5, GRIP_COLOR);
        ui.draw_calls.push(DrawCall {
            vertices: v,
            indices: i,
            texture: TextureRef::White,
        });

        let y = win.y + GRIP_H;
        let mut x = win.x + PAD;
        for (id, label) in self.buttons() {
            let w = ui.atlas.measure_text(label) + BUTTON_PAD;
            let rect = Rect::new(x, y, w, BTN_H);
            x += w + GAP;
            let resp = ui.button(id, rect, "");
            if id == FARM_BTN_ID && self.farm_active {
                tint(ui, rect, ON_TINT);
            }
            let text_w = ui.atlas.measure_text(label);
            ui.text(
                rect.x + (rect.w - text_w) / 2.0,
                rect.y + rect.h - 6.0,
                label,
                text_color(),
            );
            if resp.hovered() {
                let tip = match id {
                    FARM_BTN_ID => self.farm_tooltip(),
                    SETUP_BTN_ID => "When autofarm stops, flees, heals and returns".to_string(),
                    _ => "What auto-loot and autofarm pick up, by rarity and type".to_string(),
                };
                draw_tooltip(ui, rect, &tip);
            }
            if resp.clicked() {
                events.push(match id {
                    FARM_BTN_ID => GameEvent::ToggleAutofarm,
                    SETUP_BTN_ID => GameEvent::ToggleAutofarmWindow,
                    _ => GameEvent::ToggleLootFilterWindow,
                });
            }
        }

        let fold = Rect::new(win.x + width - PAD - FOLD_W, y, FOLD_W, BTN_H);
        let resp = ui.button(FOLD_BTN_ID, fold, "");
        if self.qol.toolbar_minimized && self.farm_active {
            tint(ui, fold, ON_TINT);
        }
        let glyph = if self.qol.toolbar_minimized { "+" } else { "-" };
        let glyph_w = ui.atlas.measure_text(glyph);
        ui.text(
            fold.x + (fold.w - glyph_w) / 2.0,
            fold.y + fold.h - 6.0,
            glyph,
            text_color(),
        );
        if resp.hovered() {
            let tip = if self.qol.toolbar_minimized {
                "Show the quality-of-life toolbar"
            } else {
                "Fold the toolbar away"
            };
            draw_tooltip(ui, fold, tip);
        }
        if resp.clicked() {
            events.extend(self.fold_events());
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

    const SCREEN: (f32, f32) = (800.0, 600.0);

    fn build_at(
        toolbar: &mut QolToolbar,
        state: &mut StateCache,
        mouse: Option<(f32, f32)>,
    ) -> Vec<GameEvent> {
        let mut ctx = UiContext::new(SCREEN.0, SCREEN.1);
        if let Some((x, y)) = mouse {
            ctx.mouse_x = x;
            ctx.mouse_y = y;
            ctx.mouse_clicked = true;
        }
        let mut ui = test_frame(&mut ctx, state);
        let mut character = Character::new();
        let data = DataTable::new();
        toolbar.build(&mut ui, &mut BuildCtx::test(&mut character, &data))
    }

    /// Where the toolbar sits on its first frame, and its width.
    fn placed(toolbar: &mut QolToolbar) -> Rect {
        let mut state = StateCache::new();
        build_at(toolbar, &mut state, None);
        toolbar.last_rect.unwrap()
    }

    /// The centre of the n-th button (from the left) of a freshly placed toolbar.
    fn button_centre(toolbar: &mut QolToolbar, n: usize) -> (f32, f32) {
        let mut ctx = UiContext::new(SCREEN.0, SCREEN.1);
        let mut state = StateCache::new();
        let ui = test_frame(&mut ctx, &mut state);
        let widths: Vec<f32> = toolbar
            .buttons()
            .iter()
            .map(|(_, label)| ui.atlas.measure_text(label) + BUTTON_PAD)
            .collect();
        let win = placed(toolbar);
        let x = win.x + PAD + widths.iter().take(n).map(|w| w + GAP).sum::<f32>() + widths[n] / 2.0;
        (x, win.y + GRIP_H + BTN_H / 2.0)
    }

    fn enabled() -> QolToolbar {
        let mut toolbar = QolToolbar::new();
        toolbar.qol.autofarm_enabled = true;
        toolbar
    }

    #[test]
    fn it_starts_in_the_bottom_right_corner() {
        let mut toolbar = enabled();
        let win = placed(&mut toolbar);
        assert!((win.x + win.w - (SCREEN.0 - SCREEN_MARGIN)).abs() <= 1.0, "{win:?}");
        assert!((win.y + win.h - (SCREEN.1 - SCREEN_MARGIN)).abs() <= 1.0, "{win:?}");
    }

    #[test]
    fn autofarm_buttons_only_show_once_it_is_enabled() {
        let off = QolToolbar::new();
        assert_eq!(off.buttons().iter().map(|b| b.0).collect::<Vec<_>>(), [FILTER_BTN_ID]);
        let on = enabled();
        assert_eq!(
            on.buttons().iter().map(|b| b.0).collect::<Vec<_>>(),
            [FARM_BTN_ID, SETUP_BTN_ID, FILTER_BTN_ID]
        );
    }

    #[test]
    fn each_button_asks_for_its_own_thing() {
        let mut toolbar = enabled();
        for (n, expected) in [(0, "farm"), (1, "setup"), (2, "filter")] {
            let (x, y) = button_centre(&mut toolbar, n);
            let mut state = StateCache::new();
            let events = build_at(&mut toolbar, &mut state, Some((x, y)));
            let got = match events.as_slice() {
                [GameEvent::ToggleAutofarm] => "farm",
                [GameEvent::ToggleAutofarmWindow] => "setup",
                [GameEvent::ToggleLootFilterWindow] => "filter",
                other => panic!("button {n}: {other:?}"),
            };
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn the_drag_strip_is_not_a_button() {
        let mut toolbar = enabled();
        let win = placed(&mut toolbar);
        let mut state = StateCache::new();
        let events = build_at(&mut toolbar, &mut state, Some((win.x + win.w / 2.0, win.y + 2.0)));
        assert!(events.is_empty(), "{events:?}");
    }

    #[test]
    fn the_fold_button_flips_the_minimized_setting() {
        let mut toolbar = enabled();
        let win = placed(&mut toolbar);
        let click = (win.x + win.w - PAD - FOLD_W / 2.0, win.y + GRIP_H + BTN_H / 2.0);
        let mut state = StateCache::new();
        match build_at(&mut toolbar, &mut state, Some(click)).as_slice() {
            [GameEvent::QolSettingsChanged { options }] => {
                assert!(options.toolbar_minimized);
                assert!(options.autofarm_enabled, "the rest of the settings ride along");
            }
            other => panic!("{other:?}"),
        }

        toolbar.qol.toolbar_minimized = true;
        let win = placed(&mut toolbar);
        let click = (win.x + win.w - PAD - FOLD_W / 2.0, win.y + GRIP_H + BTN_H / 2.0);
        let mut state = StateCache::new();
        match build_at(&mut toolbar, &mut state, Some(click)).as_slice() {
            [GameEvent::QolSettingsChanged { options }] => assert!(!options.toolbar_minimized),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn folded_it_shrinks_to_the_restore_button() {
        let mut toolbar = enabled();
        let open = placed(&mut toolbar).w;
        toolbar.qol.toolbar_minimized = true;
        assert!(toolbar.buttons().is_empty());
        let folded = placed(&mut toolbar).w;
        assert!((folded - (PAD * 2.0 + FOLD_W)).abs() < 0.01);
        assert!(folded < open);
    }

    #[test]
    fn folding_keeps_the_right_edge_where_it_was() {
        let mut toolbar = enabled();
        let mut state = StateCache::new();
        build_at(&mut toolbar, &mut state, None);
        let before = toolbar.last_rect.unwrap();
        toolbar.qol.toolbar_minimized = true;
        build_at(&mut toolbar, &mut state, None);
        let after = toolbar.last_rect.unwrap();
        assert!(
            ((before.x + before.w) - (after.x + after.w)).abs() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    #[test]
    fn the_farm_label_follows_the_state() {
        let mut toolbar = QolToolbar::new();
        assert_eq!(toolbar.farm_label(), "Autofarm: OFF");
        toolbar.farm_active = true;
        assert_eq!(toolbar.farm_label(), "Autofarm: ON");
        toolbar.hotkey = "Ctrl+F".to_string();
        assert!(toolbar.farm_tooltip().ends_with("(Ctrl+F)"));
    }
}
