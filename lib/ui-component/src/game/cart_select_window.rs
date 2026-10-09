use crate::helper::window_chrome::{draw_container, draw_sys_button, draw_titlebar, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const CART_SELECT_WINDOW_ID: WidgetId = WidgetId(1850);
const CART_SELECT_CLOSE_BTN_ID: WidgetId = WidgetId(1851);
const CART_SELECT_ROW_BASE_ID: u32 = 1855;

const TITLE_H: f32 = 17.0;
const ROW_H: f32 = 44.0;
const PAD: f32 = 6.0;
const WIN_W: f32 = 170.0;
const MINI_BTN_SIZE: f32 = 11.0;
const PREVIEW_W: f32 = 44.0;
pub const PREVIEW_SCALE: f32 = 0.4;

const CART_MODELS: [(i16, u16); 5] = [(1, 0), (2, 40), (3, 65), (4, 80), (5, 90)];

pub struct CartSelectWindow {
    open: bool,
    model_previews: Vec<(u8, [f32; 2])>,
    preview_insert_index: Option<usize>,
}

impl Default for CartSelectWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl CartSelectWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            model_previews: Vec::new(),
            preview_insert_index: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn model_previews(&self) -> &[(u8, [f32; 2])] {
        &self.model_previews
    }

    pub fn preview_insert_index(&self) -> Option<usize> {
        self.preview_insert_index
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Window for CartSelectWindow {
    fn window_size(&self) -> (f32, f32) {
        (
            WIN_W,
            TITLE_H + PAD + CART_MODELS.len() as f32 * ROW_H + PAD,
        )
    }
}

impl InGameWindow for CartSelectWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let character = &mut *ctx.character;
        let _data = ctx.data;
        self.model_previews.clear();
        self.preview_insert_index = None;
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let text_color = text_color();

        let win_h = TITLE_H + PAD + CART_MODELS.len() as f32 * ROW_H + PAD;
        let win = ui.window_at(CART_SELECT_WINDOW_ID, WIN_W, win_h, TITLE_H, 200.0, 160.0);
        ui.interact(CART_SELECT_WINDOW_ID, Rect::new(win.x, win.y, WIN_W, win_h));

        draw_titlebar(ui, win.x, win.y, WIN_W, TITLE_H);
        ui.text(
            win.x + 17.0,
            win.y + TITLE_H - 3.0,
            &rust_i18n::t!("cart_select.change_cart"),
            text_color,
        );

        let close_rect = Rect::new(
            win.x + WIN_W - MINI_BTN_SIZE - 3.0,
            win.y + (TITLE_H - MINI_BTN_SIZE) / 2.0,
            MINI_BTN_SIZE,
            MINI_BTN_SIZE,
        );
        let close_resp = ui.interact(CART_SELECT_CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (MINI_BTN_SIZE, MINI_BTN_SIZE),
            close_resp.hovered(),
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            return events;
        }

        let body_y = win.y + TITLE_H;
        let body_h = PAD + CART_MODELS.len() as f32 * ROW_H + PAD;
        draw_container(ui, win.x, body_y, WIN_W, body_h);

        for (i, (num, req_level)) in CART_MODELS.iter().enumerate() {
            let row_y = body_y + PAD + i as f32 * ROW_H;
            let row_rect = Rect::new(win.x + PAD, row_y, WIN_W - PAD * 2.0, ROW_H - 2.0);
            let unlocked = character.base_level >= *req_level;
            let resp = ui.interact(WidgetId(CART_SELECT_ROW_BASE_ID + i as u32), row_rect);
            if unlocked && resp.hovered() {
                ui.any_interactive_hovered = true;
            }

            if unlocked && resp.hovered() {
                let hover_bg = [0.3, 0.3, 0.4, 0.3];
                let (v, idx) =
                    draw::quad_vertices(row_rect.x, row_rect.y, row_rect.w, row_rect.h, hover_bg);
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: idx.to_vec(),
                    texture: TextureRef::White,
                });
            }

            if unlocked {
                let anchor_x = row_rect.x + PREVIEW_W / 2.0;
                let anchor_y = row_rect.y + ROW_H - 8.0;
                self.model_previews.push((*num as u8, [anchor_x, anchor_y]));
            }

            let label = if unlocked {
                rust_i18n::t!("cart_select.cart", num = num).to_string()
            } else {
                rust_i18n::t!("cart_select.cart_locked", num = num, level = req_level).to_string()
            };
            let label_color = if unlocked {
                text_color
            } else {
                [0.5, 0.5, 0.5, 1.0]
            };
            let text_x = row_rect.x + PREVIEW_W + 4.0;
            ui.text(text_x, row_rect.y + ROW_H / 2.0 + 1.0, &label, label_color);

            if unlocked && resp.clicked() {
                events.push(GameEvent::RequestChangeCart { num: *num });
                self.open = false;
            }
        }

        self.preview_insert_index = Some(ui.draw_calls.len());

        events
    }
}
