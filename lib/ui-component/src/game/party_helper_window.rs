use crate::helper::window_chrome::{
    draw_container, draw_footer,
    draw_sys_button, draw_titlebar, label_color, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;



pub const PARTY_HELPER_WINDOW_ID: WidgetId = WidgetId(3300);
const CLOSE_BTN_ID: WidgetId = WidgetId(3301);
const NAME_INPUT_ID: WidgetId = WidgetId(3302);
const OK_BTN_ID: WidgetId = WidgetId(3310);
const CANCEL_BTN_ID: WidgetId = WidgetId(3311);
const RADIO_BASE_ID: u32 = 3320;


const WIN_W: f32 = 150.0;
const TITLE_H: f32 = 17.0;
const FOOTER_H: f32 = 27.0;
const CLOSE_BTN_SIZE: f32 = 11.0;
const ROW_H: f32 = 16.0;

pub const MODE_CREATE: u8 = 0;
pub const MODE_INVITE: u8 = 1;
pub const MODE_SETUP: u8 = 2;
pub const MODE_ADD_FRIEND: u8 = 3;

pub struct PartyHelperWindow {
    pub open: bool,
    mode: u8,
    name_input: TextInput,
    exp_share: bool,
    item_pickup: u8,
    item_division: u8,
    editable: bool,
}

impl Default for PartyHelperWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl PartyHelperWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            mode: MODE_CREATE,
            name_input: TextInput::new(24, false),
            exp_share: false,
            item_pickup: 0,
            item_division: 0,
            editable: true,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(
        &mut self,
        mode: u8,
        exp_share: bool,
        item_pickup: u8,
        item_division: u8,
        editable: bool,
    ) {
        if self.open && self.mode == mode {
            self.open = false;
            return;
        }
        self.open = true;
        self.mode = mode;
        self.exp_share = exp_share;
        self.item_pickup = item_pickup;
        self.item_division = item_division;
        self.editable = editable;
        self.name_input.text.clear();
        self.name_input.cursor_pos = 0;
    }

    fn title(&self) -> &'static str {
        match self.mode {
            MODE_CREATE => "Create Party",
            MODE_INVITE => "Party Invitation",
            MODE_ADD_FRIEND => "Add Friend",
            _ => "Party Setup",
        }
    }

    fn radio_row(
        ui: &mut UiFrame,
        id_off: u32,
        x: f32,
        y: f32,
        label: &str,
        value: u8,
        current: &mut u8,
        editable: bool,
        tc: [f32; 4],
    ) {
        let on = *current == value;
        Self::draw_radio(ui, x, y, on);
        let color = if editable {
            tc
        } else {
            [0.53, 0.53, 0.53, 1.0]
        };
        ui.text(x + 16.0, y + 11.0, label, color);
        let rect = Rect::new(x, y, 110.0, ROW_H);
        let resp = ui.interact(WidgetId(id_off), rect);
        if resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        if editable && resp.clicked() {
            *current = value;
        }
    }

    fn draw_radio(ui: &mut UiFrame, x: f32, y: f32, on: bool) {
        let c = if on {
            [0.3, 0.6, 0.9, 1.0]
        } else {
            [0.3, 0.3, 0.35, 1.0]
        };
        let (v, i) = draw::quad_vertices(x, y + 2.0, 10.0, 10.0, c);
        ui.draw_calls.push(DrawCall {
            vertices: v.to_vec(),
            indices: i.to_vec(),
            texture: TextureRef::White,
        });
    }
}

impl Window for PartyHelperWindow {
}

impl InGameWindow for PartyHelperWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let _character = &mut *ctx.character;
        let _data = ctx.data;
        if !self.open {
            return Vec::new();
        }

        let tc = text_color();
        let lc = label_color();
        let mut events = Vec::new();

        let has_name = matches!(self.mode, MODE_CREATE | MODE_INVITE | MODE_ADD_FRIEND);
        let has_items = self.mode == MODE_CREATE || self.mode == MODE_SETUP;
        let has_exp = self.mode == MODE_SETUP;

        let mut body_h = 8.0;
        if has_name {
            body_h += 16.0 + 24.0;
        }
        if has_exp {
            body_h += 16.0 + ROW_H * 2.0 + 6.0;
        }
        if has_items {
            body_h += (16.0 + ROW_H * 2.0 + 6.0) * 2.0;
        }
        let win_h = TITLE_H + body_h + FOOTER_H;

        let win = ui.window_at(PARTY_HELPER_WINDOW_ID, WIN_W, win_h, TITLE_H, 320.0, 60.0);
        let x = win.x;
        let y = win.y;
        ui.interact(PARTY_HELPER_WINDOW_ID, Rect::new(x, y, WIN_W, win_h));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 17.0, y + TITLE_H - 3.0, self.title(), tc);

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
            self.open = false;
            return events;
        }

        let content_y = y + TITLE_H;
        draw_container(ui, x, content_y, WIN_W, body_h);

        let mut cy = content_y + 6.0;
        if has_name {
            let label = if self.mode == MODE_CREATE {
                "Party Name:"
            } else {
                "Player Name:"
            };
            ui.text(x + 10.0, cy + 11.0, label, lc);
            cy += 16.0;
            let input_rect = Rect::new(x + 10.0, cy, 120.0, 20.0);
            ui.text_input(
                NAME_INPUT_ID,
                input_rect,
                &mut self.name_input,
                TextInputBg::Gray,
            );
            cy += 24.0;
        }
        if has_exp {
            ui.text(x + 10.0, cy + 11.0, &rust_i18n::t!("party_helper.how_to_share_exp"), lc);
            cy += 16.0;
            let mut v = if self.exp_share { 1 } else { 0 };
            Self::radio_row(
                ui,
                RADIO_BASE_ID,
                x + 10.0,
                cy,
                "Each Take",
                0,
                &mut v,
                self.editable,
                tc,
            );
            cy += ROW_H;
            Self::radio_row(
                ui,
                RADIO_BASE_ID + 1,
                x + 10.0,
                cy,
                "Even Share",
                1,
                &mut v,
                self.editable,
                tc,
            );
            cy += ROW_H + 6.0;
            self.exp_share = v == 1;
        }
        if has_items {
            ui.text(x + 10.0, cy + 11.0, &rust_i18n::t!("party_helper.how_to_share_items"), lc);
            cy += 16.0;
            Self::radio_row(
                ui,
                RADIO_BASE_ID + 2,
                x + 10.0,
                cy,
                "Each Take",
                0,
                &mut self.item_pickup,
                self.editable,
                tc,
            );
            cy += ROW_H;
            Self::radio_row(
                ui,
                RADIO_BASE_ID + 3,
                x + 10.0,
                cy,
                "Party Share",
                1,
                &mut self.item_pickup,
                self.editable,
                tc,
            );
            cy += ROW_H + 6.0;

            ui.text(x + 10.0, cy + 11.0, &rust_i18n::t!("party_helper.item_sharing_type"), lc);
            cy += 16.0;
            Self::radio_row(
                ui,
                RADIO_BASE_ID + 4,
                x + 10.0,
                cy,
                "Individual",
                0,
                &mut self.item_division,
                self.editable,
                tc,
            );
            cy += ROW_H;
            Self::radio_row(
                ui,
                RADIO_BASE_ID + 5,
                x + 10.0,
                cy,
                "Shared",
                1,
                &mut self.item_division,
                self.editable,
                tc,
            );
        }

        // Footer: OK / Cancel
        let footer_y = content_y + body_h;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        let ok_rect = Rect::new(x + WIN_W - 46.0 - 42.0, footer_y + 4.0, 42.0, 20.0);
        let cancel_rect = Rect::new(x + WIN_W - 2.0 - 42.0, footer_y + 4.0, 42.0, 20.0);
        let ok = ui.button(OK_BTN_ID, ok_rect, &rust_i18n::t!("common.ok"));
        if ok.hovered() {
            ui.any_interactive_hovered = true;
        }
        let cancel = ui.button(CANCEL_BTN_ID, cancel_rect, &rust_i18n::t!("common.cancel"));
        if cancel.hovered() {
            ui.any_interactive_hovered = true;
        }

        if ok.clicked() {
            let name = self.name_input.text.trim().to_string();
            match self.mode {
                MODE_CREATE if !name.is_empty() => events.push(GameEvent::RequestPartyCreate {
                    name,
                    item_pickup_rule: self.item_pickup,
                    item_division_rule: self.item_division,
                }),
                MODE_INVITE if !name.is_empty() => {
                    events.push(GameEvent::RequestPartyInviteByName { name })
                }
                MODE_ADD_FRIEND if !name.is_empty() => {
                    events.push(GameEvent::RequestAddFriend { name })
                }
                MODE_SETUP => {
                    events.push(GameEvent::RequestPartyExpOption {
                        exp_share: self.exp_share,
                    });
                }
                _ => {}
            }
            self.open = false;
        }
        if cancel.clicked() {
            self.open = false;
        }

        events
    }
}
