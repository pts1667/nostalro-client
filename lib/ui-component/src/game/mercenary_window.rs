use crate::game::homun_window::{GaugeKind, bar};
use crate::helper::window_chrome::{
    draw_container, draw_hline, draw_sys_button, draw_titlebar,
    label_color, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::companion::{MercenaryState, aspd_display};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;



pub const MERCENARY_WINDOW_ID: WidgetId = WidgetId(3000);
const CLOSE_BTN_ID: WidgetId = WidgetId(3001);
const DISMISS_BTN_ID: WidgetId = WidgetId(3002);
const SKILL_BTN_ID: WidgetId = WidgetId(3003);


const WIN_W: f32 = 236.0;
const TITLE_H: f32 = 17.0;
const WIN_H: f32 = 190.0;
const PANEL_H: f32 = WIN_H - TITLE_H;
const LEFT_W: f32 = 88.0;
const PAD: f32 = 6.0;
const CELL_H: f32 = 21.0;
const BASELINE: f32 = 10.0;

const EXPIRE_COLOR: [f32; 4] = crate::helper::colors::RED;

pub struct MercenaryWindow {
    visible: bool,
    fired_size: (f32, f32),
    skill_size: (f32, f32),
}

impl Default for MercenaryWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl MercenaryWindow {
    pub fn new() -> Self {
        Self {
            visible: false,
            fired_size: (42.0, 20.0),
            skill_size: (42.0, 20.0),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn set_visible(&mut self, value: bool) {
        self.visible = value;
    }

    fn build_body(&mut self, ui: &mut UiFrame, merc: Option<&MercenaryState>) -> Vec<GameEvent> {
        if !self.visible {
            return Vec::new();
        }
        let Some(merc) = merc else {
            return Vec::new();
        };
        let mut events = Vec::new();
        let tc = text_color();
        let lc = label_color();

        let win = ui.window_at(MERCENARY_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 240.0, 140.0);
        let x = win.x;
        let y = win.y;
        ui.interact(MERCENARY_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + 13.0, &rust_i18n::t!("mercenary.mercenary_info"), tc);

        let sys_w = 11.0;
        let close_rect = Rect::new(x + WIN_W - 3.0 - sys_w, y + 3.0, sys_w, sys_w);
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        if close_resp.clicked() {
            self.visible = false;
        }
        draw_sys_button(
            ui,
            close_rect,
            (sys_w, sys_w),
            close_resp.hovered(),
            Some('x'),
        );

        draw_container(ui, x, y + TITLE_H, WIN_W, PANEL_H);

        // Left stat column (boxed cells).
        let stats = [
            ("Atk", merc.atk),
            ("Matk", merc.matk),
            ("Hit", merc.hit),
            ("Critical", merc.critical),
            ("Def", merc.def),
            ("Mdef", merc.mdef),
            ("Flee", merc.flee),
            ("Aspd", aspd_display(merc.aspd)),
        ];
        let cell_x = x + PAD;
        let cell_w = LEFT_W - PAD;
        let mut ly = y + TITLE_H + 2.0;
        for (label, value) in stats {
            let by = ly + BASELINE + 2.0;
            ui.text_bold(cell_x + 4.0, by, label, lc);
            ui.text_right(cell_x + cell_w - 4.0, by, &value.to_string(), tc);
            draw_hline(ui, cell_x, ly + CELL_H - 6.0, cell_w);
            ly += CELL_H;
        }

        // Right info panel.
        let rx = x + LEFT_W + PAD;
        let right_edge = x + WIN_W - PAD;
        let mut ry = y + TITLE_H + 4.0;

        ui.text(rx, ry + BASELINE, &rust_i18n::t!("mercenary.name"), tc);
        ui.text_bold(rx + 34.0, ry + BASELINE, &merc.name, lc);
        ry += 20.0;

        ui.text(rx, ry + BASELINE, &rust_i18n::t!("mercenary.level_short", level = merc.level), tc);
        let (sw, sh) = self.skill_size;
        let (fw, fh) = self.fired_size;
        let skill_rect = Rect::new(right_edge - sw, ry, sw, sh);
        let dismiss_rect = Rect::new(right_edge - sw - 4.0 - fw, ry, fw, fh);
        if ui
            .button(DISMISS_BTN_ID, dismiss_rect, &rust_i18n::t!("mercenary.dismiss"))
            .clicked()
        {
            events.push(GameEvent::RequestMercenaryCommand { command: 2 });
        }
        if ui
            .button(SKILL_BTN_ID, skill_rect, &rust_i18n::t!("mercenary.skill"))
            .clicked()
        {
            events.push(GameEvent::ToggleMercenarySkillWindow);
        }
        ry += 24.0;

        let bar_w = right_edge - rx;
        ry = bar(
            ui,
            rx,
            ry,
            bar_w,
            "HP",
            merc.hp,
            merc.max_hp,
            GaugeKind::Hp,
            tc,
            lc,
        );
        ry = bar(
            ui,
            rx,
            ry,
            bar_w,
            "SP",
            merc.sp,
            merc.max_sp,
            GaugeKind::Sp,
            tc,
            lc,
        );
        ry += 6.0;

        ui.text(rx, ry + BASELINE, &rust_i18n::t!("mercenary.expiration"), tc);
        ui.text(
            rx + 60.0,
            ry + BASELINE,
            &format_expire(merc.expire_date),
            EXPIRE_COLOR,
        );
        ry += 22.0;

        let mid = rx + (right_edge - rx) * 0.5;
        ui.text(rx, ry + BASELINE, &rust_i18n::t!("mercenary.loyalty"), tc);
        ui.text(rx + 52.0, ry + BASELINE, &merc.faith.to_string(), tc);
        ui.text(mid + 8.0, ry + BASELINE, &rust_i18n::t!("mercenary.kill"), tc);
        ry += 16.0;
        ui.text(rx, ry + BASELINE, &rust_i18n::t!("mercenary.summons"), tc);
        ui.text(rx + 52.0, ry + BASELINE, &merc.calls.to_string(), tc);
        ui.text_right(right_edge, ry + BASELINE, &merc.kills.to_string(), tc);

        events
    }
}

impl InGameWindow for MercenaryWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.visible
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.visible = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.build_body(ui, ctx.mercenary)
    }
}

impl Window for MercenaryWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

/// Formats a mercenary contract expiry (unix seconds, UTC) as `MM/DD HH:MM`.
fn format_expire(expire_date: i32) -> String {
    if expire_date <= 0 {
        return "-".to_string();
    }
    let secs = expire_date as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };

    format!("{month:02}/{day:02} {hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::format_expire;

    #[test]
    fn expire_date_formats_as_month_day_time() {
        // 2009-10-09 23:28:00 UTC
        assert_eq!(format_expire(1_255_130_880), "10/09 23:28");
        assert_eq!(format_expire(0), "-");
        assert_eq!(format_expire(-5), "-");
    }
}
