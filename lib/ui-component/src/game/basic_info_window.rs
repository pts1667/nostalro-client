use crate::helper::window_chrome::{draw_sys_button, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::character::{Character, job_class_name};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;









pub const BASIC_INFO_WINDOW_ID: WidgetId = WidgetId(1400);
const MINI_BTN_ID: WidgetId = WidgetId(1401);
const CLOSE_BTN_ID: WidgetId = WidgetId(1402);
const BTN_OPTION_ID: WidgetId = WidgetId(1410);
const BTN_STATUS_ID: WidgetId = WidgetId(1411);
const BTN_EQUIP_ID: WidgetId = WidgetId(1412);
const BTN_INVENTORY_ID: WidgetId = WidgetId(1413);
const BTN_MAP_ID: WidgetId = WidgetId(1414);
const BTN_SKILL_ID: WidgetId = WidgetId(1415);
const BTN_PARTY_ID: WidgetId = WidgetId(1416);
const BTN_CHAT_ID: WidgetId = WidgetId(1417);




const WIN_W: f32 = 280.0;
const WIN_H_LARGE: f32 = 120.0;
const WIN_H_SMALL: f32 = 33.0;
const TITLE_H: f32 = 17.0;

const HP_BAR_X: f32 = 110.0;
const HP_BAR_Y: f32 = 22.0;
const SP_BAR_Y: f32 = 43.0;
const BAR_W: f32 = 85.0;
const BAR_H: f32 = 8.0;
const EXP_BAR_Y: f32 = 77.0;
const JEXP_BAR_Y: f32 = 88.0;
const EXP_BAR_W: f32 = 100.0;
const EXP_BAR_X: f32 = 84.0;
const EXP_BAR_H: f32 = 4.0;

const BUTTONS_RIGHT: f32 = 8.0;
const BUTTONS_TOP: f32 = 18.0;
const MENU_BTN_W: f32 = 30.0;
const MENU_BTN_H: f32 = 20.0;
const MENU_BTN_SPACING_X: f32 = 4.0;
const MENU_BTN_SPACING_Y: f32 = 4.0;

const SHADOW_DRAIN_SPEED: f32 = 3.0;



pub struct BasicInfoWindow {
    hidden: bool,
    minimized: bool,
    hp_shadow: f32,
    sp_shadow: f32,
    menu_btn_size: (f32, f32),
    sys_btn_size: (f32, f32),
}

impl Default for BasicInfoWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicInfoWindow {
    pub fn new() -> Self {
        Self {
            hidden: false,
            minimized: false,
            hp_shadow: 1.0,
            sp_shadow: 1.0,
            menu_btn_size: (MENU_BTN_W, MENU_BTN_H),
            sys_btn_size: (11.0, 11.0),
        }
    }

    pub fn toggle(&mut self) {
        self.hidden = !self.hidden;
    }

    fn update_shadow(shadow: &mut f32, current: f32, delta: f32) {
        if *shadow > current {
            *shadow -= (*shadow - current) * delta * SHADOW_DRAIN_SPEED;
            if (*shadow - current).abs() < 0.001 {
                *shadow = current;
            }
        } else {
            *shadow = current;
        }
    }

    fn draw_bar_fallback(
        ui: &mut UiFrame,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        shadow_pct: f32,
        fill_color: [f32; 4],
        shadow_color: [f32; 4],
    ) {
        let bg_color = [0.15, 0.15, 0.15, 0.9];
        let (v, i) = draw::quad_vertices(x, y, w, h, bg_color);
        ui.draw_calls.push(DrawCall {
            vertices: v.to_vec(),
            indices: i.to_vec(),
            texture: TextureRef::White,
        });

        if shadow_pct > 0.0 {
            let sw = (w * shadow_pct.clamp(0.0, 1.0)).max(0.0);
            let (v, i) = draw::quad_vertices(x, y, sw, h, shadow_color);
            ui.draw_calls.push(DrawCall {
                vertices: v.to_vec(),
                indices: i.to_vec(),
                texture: TextureRef::White,
            });
        }

        if fill_pct > 0.0 {
            let fw = (w * fill_pct.clamp(0.0, 1.0)).max(0.0);
            let (v, i) = draw::quad_vertices(x, y, fw, h, fill_color);
            ui.draw_calls.push(DrawCall {
                vertices: v.to_vec(),
                indices: i.to_vec(),
                texture: TextureRef::White,
            });
        }
    }

    fn draw_exp_bar(ui: &mut UiFrame, x: f32, y: f32, fill_pct: f32) {
        crate::helper::window_chrome::draw_exp_bar(ui, x, y, EXP_BAR_W, EXP_BAR_H, fill_pct);
    }

    fn build_large(
        &mut self,
        ui: &mut UiFrame,
        character: &mut Character,
        job_class: u16,
        win: Rect,
    ) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let tc = text_color();
        let x = win.x;
        let y = win.y;
        let delta = ui.elapsed_secs;

        crate::helper::fallback::panel(ui, x, y, WIN_W, WIN_H_LARGE);

        let close_rect = Rect::new(x + 4.0, y + 3.0, self.sys_btn_size.0, self.sys_btn_size.1);
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (self.sys_btn_size.0, self.sys_btn_size.1),
            close_resp.hovered(),
            None,
        );

        ui.text(x + 18.0, y + 13.0, &rust_i18n::t!("basic_info.basic_info"), tc);

        let mini_rect = Rect::new(
            x + WIN_W - 2.0 - self.sys_btn_size.0,
            y + 3.0,
            self.sys_btn_size.0,
            self.sys_btn_size.1,
        );
        let mini_resp = ui.interact(MINI_BTN_ID, mini_rect);
        if mini_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        if mini_resp.clicked() {
            self.minimized = true;
        }
        draw_sys_button(
            ui,
            mini_rect,
            (self.sys_btn_size.0, self.sys_btn_size.1),
            mini_resp.hovered(),
            Some('_'),
        );

        let name = if character.name.is_empty() {
            "Unknown"
        } else {
            &character.name
        };
        ui.text(x + 10.0, y + 30.0, name, tc);

        let job_name = job_class_name(job_class);
        ui.text(x + 10.0, y + 43.0, job_name, tc);

        let hp_pct = character.hp_percentage();
        Self::update_shadow(&mut self.hp_shadow, hp_pct, delta);
        let is_red = hp_pct < 0.25;

        ui.text(x + 90.0, y + 39.0, &rust_i18n::t!("basic_info.hp"), tc);
        let hp_fill = if is_red {
            [0.8, 0.2, 0.2, 1.0]
        } else {
            [0.2, 0.4, 0.8, 1.0]
        };
        Self::draw_bar_fallback(
            ui,
            x + HP_BAR_X,
            y + HP_BAR_Y,
            BAR_W,
            BAR_H,
            hp_pct,
            self.hp_shadow,
            hp_fill,
            [0.4, 0.4, 0.6, 0.7],
        );
        let hp_text = format!("{} / {}", character.hp, character.max_hp);
        ui.text_centered(
            x + HP_BAR_X,
            y + HP_BAR_Y + BAR_H * 2.0 + 1.0,
            BAR_W,
            &hp_text,
            tc,
        );

        let hp_bar_rect = Rect::new(x + HP_BAR_X, y + HP_BAR_Y, BAR_W, BAR_H + 10.0);
        if hp_bar_rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            ui.tooltip(
                ui.ctx.mouse_x + 10.0,
                ui.ctx.mouse_y,
                &format!("{:.1}%", hp_pct * 100.0),
            );
        }

        let sp_pct = character.sp_percentage();
        Self::update_shadow(&mut self.sp_shadow, sp_pct, delta);

        ui.text(x + 90.0, y + 59.0, &rust_i18n::t!("basic_info.sp"), tc);
        let sp_fill = [0.2, 0.7, 0.3, 1.0];
        Self::draw_bar_fallback(
            ui,
            x + HP_BAR_X,
            y + SP_BAR_Y,
            BAR_W,
            BAR_H,
            sp_pct,
            self.sp_shadow,
            sp_fill,
            [0.3, 0.5, 0.3, 0.7],
        );
        let sp_text = format!("{} / {}", character.sp, character.max_sp);
        ui.text_centered(
            x + HP_BAR_X,
            y + SP_BAR_Y + BAR_H * 2.0 + 1.0,
            BAR_W,
            &sp_text,
            tc,
        );

        let sp_bar_rect = Rect::new(x + HP_BAR_X, y + SP_BAR_Y, BAR_W, BAR_H + 10.0);
        if sp_bar_rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            ui.tooltip(
                ui.ctx.mouse_x + 10.0,
                ui.ctx.mouse_y,
                &format!("{:.1}%", sp_pct * 100.0),
            );
        }

        let blvl_text = format!("Base Lv. {}", character.base_level);
        ui.text(x + 15.0, y + 80.0, &blvl_text, tc);
        let base_exp_pct = character.base_exp_percentage();
        Self::draw_exp_bar(ui, x + EXP_BAR_X, y + EXP_BAR_Y, base_exp_pct);
        let exp_text = format!("{:.1}%", (base_exp_pct * 1000.0).floor() * 0.1);

        let exp_bar_rect = Rect::new(
            x + EXP_BAR_X,
            y + EXP_BAR_Y,
            EXP_BAR_W + 2.0,
            EXP_BAR_H + 2.0,
        );
        if exp_bar_rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            ui.tooltip(ui.ctx.mouse_x + 10.0, ui.ctx.mouse_y, &exp_text);
        }

        let jlvl_text = format!("Job Lv. {}", character.job_level);
        ui.text(x + 15.0, y + 93.0, &jlvl_text, tc);
        let job_exp_pct = character.job_exp_percentage();
        Self::draw_exp_bar(ui, x + EXP_BAR_X, y + JEXP_BAR_Y, job_exp_pct);
        let jexp_text = format!("{:.1}%", (job_exp_pct * 1000.0).floor() * 0.1);

        let jexp_bar_rect = Rect::new(
            x + EXP_BAR_X,
            y + JEXP_BAR_Y,
            EXP_BAR_W + 2.0,
            EXP_BAR_H + 2.0,
        );
        if jexp_bar_rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            ui.tooltip(ui.ctx.mouse_x + 10.0, ui.ctx.mouse_y, &jexp_text);
        }

        let weight = character.inventory.weight;
        let max_weight = character.inventory.max_weight;
        let weight_over = max_weight > 0 && weight as f32 / max_weight as f32 >= 0.5;
        let weight_color = if weight_over {
            crate::helper::colors::RED
        } else {
            tc
        };
        let weight_text = rust_i18n::t!("basic_info.weight", current = weight / 10, max = max_weight / 10).to_string();
        let weight_width = ui.atlas.measure_text(&weight_text);
        ui.text(x + 5.0, y + 115.0, &weight_text, weight_color);

        let zeny_text = rust_i18n::t!("basic_info.zeny", zeny = format_zeny(character.inventory.zeny)).to_string();
        ui.text(x + 5.0 + weight_width + 8.0, y + 115.0, &zeny_text, tc);

        let btn_w = self.menu_btn_size.0;
        let btn_h = self.menu_btn_size.1;
        let col2_x = x + WIN_W - BUTTONS_RIGHT - btn_w;
        let col1_x = col2_x - MENU_BTN_SPACING_X - btn_w;

        let row_y = |row: usize| y + BUTTONS_TOP + row as f32 * (btn_h + MENU_BTN_SPACING_Y);

        let menu_buttons = [
            (BTN_OPTION_ID, col1_x, row_y(0), rust_i18n::t!("basic_info.menu_option"), rust_i18n::t!("basic_info.tip_option")),
            (BTN_STATUS_ID, col2_x, row_y(0), rust_i18n::t!("basic_info.menu_status"), rust_i18n::t!("basic_info.tip_status")),
            (BTN_EQUIP_ID, col1_x, row_y(1), rust_i18n::t!("basic_info.menu_equipment"), rust_i18n::t!("basic_info.tip_equipment")),
            (BTN_INVENTORY_ID, col2_x, row_y(1), rust_i18n::t!("basic_info.menu_inventory"), rust_i18n::t!("basic_info.tip_inventory")),
            (BTN_MAP_ID, col1_x, row_y(2), rust_i18n::t!("basic_info.menu_map"), rust_i18n::t!("basic_info.tip_map")),
            (BTN_SKILL_ID, col2_x, row_y(2), rust_i18n::t!("basic_info.menu_skills"), rust_i18n::t!("basic_info.tip_skills")),
            (BTN_PARTY_ID, col1_x, row_y(3), rust_i18n::t!("basic_info.menu_party"), rust_i18n::t!("basic_info.tip_party")),
            (BTN_CHAT_ID, col2_x, row_y(3), rust_i18n::t!("basic_info.menu_chat"), rust_i18n::t!("basic_info.tip_chat")),
        ];

        for (id, bx, by, label, tooltip_text) in menu_buttons {
            let rect = Rect::new(bx, by, btn_w, btn_h);
            let resp = ui.button(id, rect, &label);
            if resp.clicked() {
                match id {
                    BTN_OPTION_ID => events.push(GameEvent::ToggleSystemMenu),
                    BTN_EQUIP_ID => events.push(GameEvent::ToggleEquipment),
                    BTN_INVENTORY_ID => events.push(GameEvent::ToggleInventory),
                    BTN_MAP_ID => events.push(GameEvent::ToggleMinimap),
                    BTN_SKILL_ID => events.push(GameEvent::ToggleSkills),
                    BTN_STATUS_ID => events.push(GameEvent::ToggleStatusWindow),
                    BTN_PARTY_ID => events.push(GameEvent::TogglePartyWindow),
                    BTN_CHAT_ID => events.push(GameEvent::ToggleChatRoomCreate),
                    _ => {}
                }
            }
            if resp.hovered() {
                ui.tooltip(ui.ctx.mouse_x, ui.ctx.mouse_y, &tooltip_text);
            }
        }

        events
    }

    fn build_small(
        &mut self,
        ui: &mut UiFrame,
        character: &mut Character,
        job_class: u16,
        win: Rect,
    ) -> Vec<GameEvent> {
        let tc = text_color();
        let x = win.x;
        let y = win.y;

        crate::helper::fallback::panel(ui, x, y, WIN_W, WIN_H_SMALL);

        let name = if character.name.is_empty() {
            "Unknown"
        } else {
            &character.name
        };
        ui.text(x + 18.0, y + 12.0, name, tc);

        let mini_rect = Rect::new(
            x + WIN_W - 2.0 - self.sys_btn_size.0,
            y + 3.0,
            self.sys_btn_size.0,
            self.sys_btn_size.1,
        );
        let mini_resp = ui.interact(MINI_BTN_ID, mini_rect);
        if mini_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        if mini_resp.clicked() {
            self.minimized = false;
        }
        draw_sys_button(
            ui,
            mini_rect,
            (self.sys_btn_size.0, self.sys_btn_size.1),
            mini_resp.hovered(),
            Some('+'),
        );

        let job_name = job_class_name(job_class);
        let exp_pct = character.base_exp_percentage() * 100.0;
        let info_text = format!(
            "Lv.{} / {} / Lv.{} / Exp. {:.1}%",
            character.base_level, job_name, character.job_level, exp_pct
        );
        ui.text_right(x + WIN_W - 18.0, y + 12.0, &info_text, tc);

        let hp_sp_text = format!(
            "HP. {} / {} | SP. {} / {}",
            character.hp, character.max_hp, character.sp, character.max_sp
        );
        ui.text_right(x + WIN_W - 5.0, y + 28.0, &hp_sp_text, tc);

        Vec::new()
    }
}

impl Window for BasicInfoWindow {

    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H_LARGE)
    }
}

impl InGameWindow for BasicInfoWindow {
    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let job_class = ctx.job_class;
        let character = &mut *ctx.character;
        let _data = ctx.data;
        if self.hidden {
            return Vec::new();
        }

        let win_h = if self.minimized {
            WIN_H_SMALL
        } else {
            WIN_H_LARGE
        };
        let win = ui.window_at(BASIC_INFO_WINDOW_ID, WIN_W, win_h, TITLE_H, 0.0, 0.0);

        let win_rect = Rect::new(win.x, win.y, WIN_W, win_h);
        ui.interact(BASIC_INFO_WINDOW_ID, win_rect);

        let events = if self.minimized {
            self.build_small(ui, character, job_class, win)
        } else {
            self.build_large(ui, character, job_class, win)
        };

        events
    }
}

fn format_zeny(value: i32) -> String {
    if value < 0 {
        return format!("-{}", format_zeny(-value));
    }
    let s = value.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
