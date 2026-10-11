//! The autofarm window: the start/stop switch and how autofarm looks after
//! itself — when it stops, flees or returns, and what it heals with.

use crate::helper::window_chrome::{draw_sys_button, draw_titlebar, label_color, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::automation::{
    BUTTERFLY_WING, FarmOptions, HEAL_ITEMS, QolOptions, SELF_HEAL_SKILLS, SP_ITEMS,
};
use ragnarok_game::character::Character;
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const AUTOFARM_WINDOW_ID: WidgetId = WidgetId(6200);
const CLOSE_BTN_ID: WidgetId = WidgetId(6201);
const START_BTN_ID: WidgetId = WidgetId(6202);
const HEAVY_CB_ID: WidgetId = WidgetId(6203);
const HEAVY_RETURN_CB_ID: WidgetId = WidgetId(6204);
const HURT_CB_ID: WidgetId = WidgetId(6205);
const HURT_RETURN_CB_ID: WidgetId = WidgetId(6206);
const HEAL_ITEMS_CB_ID: WidgetId = WidgetId(6207);
const HEAL_SKILLS_CB_ID: WidgetId = WidgetId(6208);
const SP_CB_ID: WidgetId = WidgetId(6209);
const HEAVY_MINUS_ID: WidgetId = WidgetId(6210);
const HEAVY_PLUS_ID: WidgetId = WidgetId(6211);
const HURT_MINUS_ID: WidgetId = WidgetId(6212);
const HURT_PLUS_ID: WidgetId = WidgetId(6213);
const HEAL_MINUS_ID: WidgetId = WidgetId(6214);
const HEAL_PLUS_ID: WidgetId = WidgetId(6215);
const SP_MINUS_ID: WidgetId = WidgetId(6216);
const SP_PLUS_ID: WidgetId = WidgetId(6217);

const WIN_W: f32 = 340.0;
const TITLE_H: f32 = 20.0;
const CLOSE_SIZE: f32 = 11.0;
const ROW_H: f32 = 22.0;
const ROW_COUNT: usize = 13;
const PAD: f32 = 8.0;
const WIN_H: f32 = TITLE_H + PAD + ROW_COUNT as f32 * ROW_H + PAD;
const CB_SIZE: f32 = 11.0;
const INDENT: f32 = 16.0;
const STEP_BTN: f32 = 16.0;
const STEP_VALUE_W: f32 = 38.0;
/// Left edge of the percentage steppers, from the window's left edge.
const STEP_X: f32 = WIN_W - PAD - STEP_BTN * 2.0 - STEP_VALUE_W - 4.0;

/// The rows, top to bottom.
const ROW_START: usize = 0;
const ROW_STOP_HEADER: usize = 1;
const ROW_HEAVY: usize = 2;
const ROW_HEAVY_RETURN: usize = 3;
const ROW_HURT: usize = 4;
const ROW_HURT_RETURN: usize = 5;
const ROW_RECOVER_HEADER: usize = 6;
const ROW_HEAL_BELOW: usize = 7;
const ROW_HEAL_ITEMS: usize = 8;
const ROW_HEAL_SKILLS: usize = 9;
const ROW_SP: usize = 10;
const ROW_SUPPLIES: usize = 11;
const ROW_SKILL: usize = 12;

/// What the steppers allow, as (min, max, step).
const HEAVY_RANGE: (u8, u8, u8) = (50, 95, 5);
const HURT_RANGE: (u8, u8, u8) = (5, 60, 5);
const HEAL_RANGE: (u8, u8, u8) = (10, 90, 5);
const SP_RANGE: (u8, u8, u8) = (5, 80, 5);

pub struct AutofarmWindow {
    pub open: bool,
    /// The current settings; the client copies them in before each build.
    pub qol: QolOptions,
    /// Whether autofarm is running.
    pub farm_active: bool,
    /// How the autofarm hotkey reads; empty when unbound.
    pub hotkey: String,
}

impl Default for AutofarmWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// How many of the items in `ids` the character carries.
fn carried(character: &Character, ids: &[u16]) -> i64 {
    character
        .inventory
        .all_items()
        .iter()
        .filter(|item| ids.contains(&item.item_id))
        .map(|item| i64::from(item.count))
        .sum()
}

impl AutofarmWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            qol: QolOptions::default(),
            farm_active: false,
            hotkey: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    fn start_label(&self) -> String {
        if self.farm_active {
            return "Stop autofarm".to_string();
        }
        if self.hotkey.is_empty() {
            "Start autofarm".to_string()
        } else {
            format!("Start autofarm  ({})", self.hotkey)
        }
    }
}

/// A `-  NN%  +` control on the row at `y`; returns whether the value changed.
fn stepper(
    ui: &mut UiFrame,
    ids: (WidgetId, WidgetId),
    x: f32,
    y: f32,
    value: &mut u8,
    (min, max, step): (u8, u8, u8),
) -> bool {
    let before = *value;
    let minus = Rect::new(x, y, STEP_BTN, STEP_BTN);
    let plus = Rect::new(x + STEP_BTN + STEP_VALUE_W, y, STEP_BTN, STEP_BTN);
    if ui.button(ids.0, minus, "-").clicked() {
        *value = value.saturating_sub(step).max(min);
    }
    if ui.button(ids.1, plus, "+").clicked() {
        *value = value.saturating_add(step).min(max);
    }
    *value = (*value).clamp(min, max);
    let text = format!("{}%", *value);
    let text_w = ui.atlas.measure_text(&text);
    ui.text(
        x + STEP_BTN + (STEP_VALUE_W - text_w) / 2.0,
        y + STEP_BTN - 3.0,
        &text,
        text_color(),
    );
    *value != before
}

impl Window for AutofarmWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for AutofarmWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let default_x = ((ui.ctx.screen_width - WIN_W) / 2.0).floor();
        let default_y = ((ui.ctx.screen_height - WIN_H) / 2.0).floor();
        let win = ui.window_at(AUTOFARM_WINDOW_ID, WIN_W, WIN_H, TITLE_H, default_x, default_y);
        ui.interact(AUTOFARM_WINDOW_ID, Rect::new(win.x, win.y, WIN_W, WIN_H));

        crate::helper::fallback::window_body(ui, win.x, win.y + TITLE_H, WIN_W, WIN_H - TITLE_H);
        draw_titlebar(ui, win.x, win.y, WIN_W, TITLE_H);
        ui.text(win.x + 20.0, win.y + TITLE_H - 5.0, "Autofarm", text_color());

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
        let cb_y = |n: usize| row_y(n) + (ROW_H - CB_SIZE) / 2.0 - 2.0;
        let text_y = |n: usize| row_y(n) + 13.0;
        let step_y = |n: usize| row_y(n) + (ROW_H - STEP_BTN) / 2.0 - 2.0;
        let x0 = win.x + 10.0;
        let label = text_color();

        let mut farm = self.qol.farm;
        let before: FarmOptions = farm;

        // Start / stop.
        let start = Rect::new(win.x + PAD, row_y(ROW_START), WIN_W - PAD * 2.0, ROW_H - 2.0);
        let start_label = self.start_label();
        let resp = ui.button(START_BTN_ID, start, &start_label);
        if self.farm_active {
            let (v, i) = ragnarok_ui::draw::rounded_rect(
                start.x,
                start.y,
                start.w,
                start.h,
                3.0,
                [0.15, 0.75, 0.25, 0.55],
            );
            ui.draw_calls.push(ragnarok_ui::draw::DrawCall {
                vertices: v,
                indices: i,
                texture: ragnarok_ui::draw::TextureRef::White,
            });
            let w = ui.atlas.measure_text(&start_label);
            ui.text(
                start.x + (start.w - w) / 2.0,
                start.y + start.h - 6.0,
                &start_label,
                label,
            );
        }
        if resp.clicked() {
            events.push(GameEvent::ToggleAutofarm);
        }

        let check = |ui: &mut UiFrame, id: WidgetId, x: f32, n: usize, text: &str, value: &mut bool| {
            let rect = Rect::new(x, cb_y(n), CB_SIZE, CB_SIZE);
            ui.checkbox(id, rect, value);
            ui.text(rect.x + CB_SIZE + 4.0, text_y(n), text, label);
        };

        // When to stop.
        ui.text(x0, text_y(ROW_STOP_HEADER), "When to stop", label_color());
        check(ui, HEAVY_CB_ID, x0, ROW_HEAVY, "Stop when weight is over", &mut farm.stop_when_heavy);
        stepper(
            ui,
            (HEAVY_MINUS_ID, HEAVY_PLUS_ID),
            win.x + STEP_X,
            step_y(ROW_HEAVY),
            &mut farm.heavy_percent,
            HEAVY_RANGE,
        );
        check(
            ui,
            HEAVY_RETURN_CB_ID,
            x0 + INDENT,
            ROW_HEAVY_RETURN,
            "Return to the save point (Butterfly Wing)",
            &mut farm.return_when_heavy,
        );
        check(ui, HURT_CB_ID, x0, ROW_HURT, "Flee when HP is below", &mut farm.flee_when_hurt);
        stepper(
            ui,
            (HURT_MINUS_ID, HURT_PLUS_ID),
            win.x + STEP_X,
            step_y(ROW_HURT),
            &mut farm.flee_hp_percent,
            HURT_RANGE,
        );
        check(
            ui,
            HURT_RETURN_CB_ID,
            x0 + INDENT,
            ROW_HURT_RETURN,
            "Return to the save point (Butterfly Wing)",
            &mut farm.return_when_hurt,
        );

        // Recovery.
        ui.text(x0, text_y(ROW_RECOVER_HEADER), "Recovery", label_color());
        ui.text(x0 + CB_SIZE + 4.0, text_y(ROW_HEAL_BELOW), "Heal when HP is below", label);
        stepper(
            ui,
            (HEAL_MINUS_ID, HEAL_PLUS_ID),
            win.x + STEP_X,
            step_y(ROW_HEAL_BELOW),
            &mut farm.heal_hp_percent,
            HEAL_RANGE,
        );
        check(
            ui,
            HEAL_ITEMS_CB_ID,
            x0 + INDENT,
            ROW_HEAL_ITEMS,
            "Use healing items (Red Potion and up)",
            &mut farm.heal_with_items,
        );
        check(
            ui,
            HEAL_SKILLS_CB_ID,
            x0 + INDENT,
            ROW_HEAL_SKILLS,
            "Cast a healing skill on yourself",
            &mut farm.heal_with_skills,
        );
        check(ui, SP_CB_ID, x0, ROW_SP, "Use SP items when SP is below", &mut farm.restore_sp);
        stepper(
            ui,
            (SP_MINUS_ID, SP_PLUS_ID),
            win.x + STEP_X,
            step_y(ROW_SP),
            &mut farm.restore_sp_percent,
            SP_RANGE,
        );

        // What the character has to work with.
        let character = &*ctx.character;
        let wings = carried(character, &[BUTTERFLY_WING]);
        let heal = carried(character, &HEAL_ITEMS);
        let sp = carried(character, &SP_ITEMS);
        let warn = [0.75, 0.1, 0.1, 1.0];
        let supplies = format!("Carrying: {wings} Butterfly Wing, {heal} healing, {sp} SP items");
        let short = ((farm.return_when_heavy || farm.return_when_hurt) && wings == 0)
            || (farm.heal_with_items && heal == 0)
            || (farm.restore_sp && sp == 0);
        ui.text(x0, text_y(ROW_SUPPLIES), &supplies, if short { warn } else { label_color() });
        let heal_skill = SELF_HEAL_SKILLS
            .iter()
            .filter_map(|skill| character.skills.get_skill(*skill))
            .find(|data| data.level > 0);
        let skill_text = match heal_skill {
            Some(data) => format!("Healing skill: {} level {}", data.skill.to_name(), data.selected_level.max(1)),
            None => "Healing skill: none learned".to_string(),
        };
        ui.text(x0, text_y(ROW_SKILL), &skill_text, label_color());

        if farm != before {
            let mut options = self.qol;
            options.farm = farm;
            self.qol = options;
            events.push(GameEvent::QolSettingsChanged { options });
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_game::data_table::DataTable;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    fn build_with(
        win: &mut AutofarmWindow,
        character: &mut Character,
        state: &mut StateCache,
        mouse: Option<(f32, f32)>,
    ) -> Vec<GameEvent> {
        let mut ctx = UiContext::new(800.0, 600.0);
        if let Some((x, y)) = mouse {
            ctx.mouse_x = x;
            ctx.mouse_y = y;
            ctx.mouse_clicked = true;
        }
        let mut ui = test_frame(&mut ctx, state);
        let data = DataTable::new();
        win.build(&mut ui, &mut BuildCtx::test(character, &data))
    }

    fn click(win: &mut AutofarmWindow, x: f32, y: f32) -> Vec<GameEvent> {
        let mut character = Character::new();
        let mut state = StateCache::new();
        build_with(win, &mut character, &mut state, Some((x, y)))
    }

    fn origin() -> (f32, f32) {
        ((800.0 - WIN_W) / 2.0, (600.0 - WIN_H) / 2.0)
    }

    fn row_y(n: usize) -> f32 {
        origin().1 + TITLE_H + PAD + n as f32 * ROW_H
    }

    /// Centre of the checkbox starting at `x` (from the window's edge) on a row.
    fn click_check(win: &mut AutofarmWindow, row: usize, x: f32) -> Vec<GameEvent> {
        click(
            win,
            origin().0 + x + CB_SIZE / 2.0,
            row_y(row) + (ROW_H - CB_SIZE) / 2.0 - 2.0 + CB_SIZE / 2.0,
        )
    }

    fn click_step(win: &mut AutofarmWindow, row: usize, plus: bool) -> Vec<GameEvent> {
        let x = STEP_X + if plus { STEP_BTN + STEP_VALUE_W } else { 0.0 } + STEP_BTN / 2.0;
        click(
            win,
            origin().0 + x,
            row_y(row) + (ROW_H - STEP_BTN) / 2.0 - 2.0 + STEP_BTN / 2.0,
        )
    }

    fn open_window() -> AutofarmWindow {
        let mut win = AutofarmWindow::new();
        win.toggle();
        win
    }

    fn only_options(events: &[GameEvent]) -> QolOptions {
        match events {
            [GameEvent::QolSettingsChanged { options }] => *options,
            other => panic!("unexpected events: {other:?}"),
        }
    }

    #[test]
    fn it_draws_nothing_while_closed() {
        let mut win = AutofarmWindow::new();
        let events = click(&mut win, 400.0, 300.0);
        assert!(events.is_empty());
    }

    #[test]
    fn the_start_button_toggles_autofarm() {
        let mut win = open_window();
        let events = click(&mut win, origin().0 + WIN_W / 2.0, row_y(ROW_START) + 8.0);
        assert!(matches!(events.as_slice(), [GameEvent::ToggleAutofarm]), "{events:?}");
    }

    #[test]
    fn the_start_label_follows_state_and_hotkey() {
        let mut win = AutofarmWindow::new();
        assert_eq!(win.start_label(), "Start autofarm");
        win.hotkey = "Ctrl+F".to_string();
        assert_eq!(win.start_label(), "Start autofarm  (Ctrl+F)");
        win.farm_active = true;
        assert_eq!(win.start_label(), "Stop autofarm");
    }

    #[test]
    fn each_checkbox_flips_its_own_option() {
        let defaults = FarmOptions::default();
        let rows: [(usize, f32, fn(&FarmOptions) -> bool); 6] = [
            (ROW_HEAVY, 10.0, |f| f.stop_when_heavy),
            (ROW_HEAVY_RETURN, 10.0 + INDENT, |f| f.return_when_heavy),
            (ROW_HURT, 10.0, |f| f.flee_when_hurt),
            (ROW_HURT_RETURN, 10.0 + INDENT, |f| f.return_when_hurt),
            (ROW_HEAL_ITEMS, 10.0 + INDENT, |f| f.heal_with_items),
            (ROW_HEAL_SKILLS, 10.0 + INDENT, |f| f.heal_with_skills),
        ];
        for (row, x, get) in rows {
            let mut win = open_window();
            let options = only_options(&click_check(&mut win, row, x));
            assert_ne!(get(&options.farm), get(&defaults), "row {row} flipped its option");
            let flipped = [
                options.farm.stop_when_heavy != defaults.stop_when_heavy,
                options.farm.return_when_heavy != defaults.return_when_heavy,
                options.farm.flee_when_hurt != defaults.flee_when_hurt,
                options.farm.return_when_hurt != defaults.return_when_hurt,
                options.farm.heal_with_items != defaults.heal_with_items,
                options.farm.heal_with_skills != defaults.heal_with_skills,
            ];
            assert_eq!(flipped.iter().filter(|&&f| f).count(), 1, "row {row}: {flipped:?}");
        }
        let mut win = open_window();
        let options = only_options(&click_check(&mut win, ROW_SP, 10.0));
        assert_ne!(options.farm.restore_sp, defaults.restore_sp);
    }

    #[test]
    fn the_steppers_move_by_their_step_within_their_range() {
        let mut win = open_window();
        let up = only_options(&click_step(&mut win, ROW_HEAVY, true));
        assert_eq!(up.farm.heavy_percent, 85);
        let down = only_options(&click_step(&mut win, ROW_HURT, false));
        assert_eq!(down.farm.flee_hp_percent, 20);
        let up = only_options(&click_step(&mut win, ROW_HEAL_BELOW, true));
        assert_eq!(up.farm.heal_hp_percent, 55);
        let down = only_options(&click_step(&mut win, ROW_SP, false));
        assert_eq!(down.farm.restore_sp_percent, 20);
    }

    #[test]
    fn steppers_stop_at_the_ends_of_their_range() {
        let mut win = open_window();
        win.qol.farm.heavy_percent = HEAVY_RANGE.1;
        assert!(click_step(&mut win, ROW_HEAVY, true).is_empty(), "already at the top");

        let mut win = open_window();
        win.qol.farm.flee_hp_percent = HURT_RANGE.0;
        assert!(click_step(&mut win, ROW_HURT, false).is_empty(), "already at the bottom");
    }

    #[test]
    fn a_change_carries_the_other_settings_along() {
        let mut win = open_window();
        win.qol.auto_loot = true;
        win.qol.autofarm_enabled = true;
        win.qol.loot_filter.min_rarity = ragnarok_game::automation::Rarity::Rare;
        let options = only_options(&click_step(&mut win, ROW_HEAVY, true));
        assert!(options.auto_loot && options.autofarm_enabled);
        assert_eq!(options.loot_filter.min_rarity, ragnarok_game::automation::Rarity::Rare);
    }

    #[test]
    fn an_idle_frame_changes_nothing() {
        let mut win = open_window();
        let mut character = Character::new();
        let mut state = StateCache::new();
        assert!(build_with(&mut win, &mut character, &mut state, None).is_empty());
    }

    #[test]
    fn carried_counts_every_stack() {
        let mut character = Character::new();
        assert_eq!(carried(&character, &[BUTTERFLY_WING]), 0);
        for (index, count) in [(1, 3), (2, 4)] {
            character.inventory.add_item(ragnarok_game::item::Item {
                index,
                item_id: BUTTERFLY_WING,
                item_type: models::enums::item::ItemType::Usable,
                count,
                is_identified: true,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
                location: 0,
                wear_state: 0,
                name: "Butterfly Wing".to_string(),
                resource_name: None,
            });
        }
        assert_eq!(carried(&character, &[BUTTERFLY_WING]), 7);
        assert_eq!(carried(&character, &HEAL_ITEMS), 0);
    }
}
