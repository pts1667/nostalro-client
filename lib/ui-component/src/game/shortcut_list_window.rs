use crate::helper::window_chrome::{
    draw_container, draw_footer, draw_sys_button,
    draw_titlebar, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;


pub const SHORTCUT_LIST_WINDOW_ID: WidgetId = WidgetId(3600);
const CLOSE_BTN_ID: WidgetId = WidgetId(3601);
const VIEW_BTN_ID: WidgetId = WidgetId(3602);
const ROW_INPUT_BASE: u32 = 3610;

pub const SLOT_COUNT: usize = 10;
const WIN_W: f32 = 230.0;
const TITLE_H: f32 = 17.0;
const FOOTER_H: f32 = 30.0;
const ROW_H: f32 = 22.0;
const PAD: f32 = 8.0;
const LABEL_W: f32 = 52.0;
const CLOSE_BTN_SIZE: f32 = 11.0;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;

fn win_h() -> f32 {
    TITLE_H + PAD + SLOT_COUNT as f32 * ROW_H + PAD + FOOTER_H
}

fn slot_hotkey_digit(slot: usize) -> u8 {
    if slot == 9 { 0 } else { slot as u8 + 1 }
}

pub struct ShortcutListWindow {
    open: bool,
    slots: [TextInput; SLOT_COUNT],
    committed: Vec<String>,
    btn_size: (f32, f32),
}

impl Default for ShortcutListWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutListWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            slots: std::array::from_fn(|_| TextInput::new(23, false)),
            committed: Vec::new(),
            btn_size: (FALLBACK_BTN_W, FALLBACK_BTN_H),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn set_bindings(&mut self, commands: &[String]) {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let text = commands.get(i).cloned().unwrap_or_default();
            slot.cursor_pos = text.chars().count();
            slot.text = text;
        }
        self.committed = self.slots.iter().map(|s| s.text.clone()).collect();
    }
}

impl Window for ShortcutListWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, win_h())
    }
}

impl InGameWindow for ShortcutListWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let _character = &mut *ctx.character;
        let _data = ctx.data;
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let tc = text_color();
        let bg = TextInputBg::Default;

        let h = win_h();
        let win = ui.window_at(SHORTCUT_LIST_WINDOW_ID, WIN_W, h, TITLE_H, 520.0, 160.0);
        let (x, y) = (win.x, win.y);
        ui.interact(SHORTCUT_LIST_WINDOW_ID, Rect::new(x, y, WIN_W, h));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + TITLE_H - 3.0, "Shortcut List", tc);

        let close_rect = Rect::new(
            x + WIN_W - CLOSE_BTN_SIZE - 3.0,
            y + (TITLE_H - CLOSE_BTN_SIZE) / 2.0,
            CLOSE_BTN_SIZE,
            CLOSE_BTN_SIZE,
        );
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
            close_resp.hovered(),
            Some('x'),
        );
        if close_resp.clicked() {
            self.close();
            return events;
        }

        let body_y = y + TITLE_H;
        let body_h = PAD + SLOT_COUNT as f32 * ROW_H + PAD;
        draw_container(ui, x, body_y, WIN_W, body_h);

        let field_x = x + PAD + LABEL_W;
        let field_w = WIN_W - PAD * 2.0 - LABEL_W;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let row_y = body_y + PAD + i as f32 * ROW_H;
            ui.text(
                x + PAD,
                row_y + 12.0,
                &format!("Alt + {}", slot_hotkey_digit(i)),
                tc,
            );
            ui.text_input(
                WidgetId(ROW_INPUT_BASE + i as u32),
                Rect::new(field_x, row_y, field_w, 16.0),
                slot,
                bg,
            );
        }

        let footer_y = y + h - FOOTER_H;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        let (bw, bh) = self.btn_size;
        let ok_rect = Rect::new(
            x + WIN_W - PAD - bw,
            footer_y + (FOOTER_H - bh) / 2.0,
            bw,
            bh,
        );
        if ui.button(VIEW_BTN_ID, ok_rect, "view").clicked() {
            events.push(GameEvent::ToggleEmotionWindow);
        }

        let current: Vec<String> = self.slots.iter().map(|s| s.text.clone()).collect();
        if current != self.committed {
            self.committed = current.clone();
            events.push(GameEvent::ShortcutBindingsChanged(current));
        }

        events
    }
}
