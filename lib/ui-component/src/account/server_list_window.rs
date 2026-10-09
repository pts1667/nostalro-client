use crate::Window;
use ragnarok_game::event::{GameEvent, ServerInfo};
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;



const FALLBACK_WIN_W: f32 = 280.0;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;

const ROW_H: f32 = 17.0;

pub const SERVER_LIST_WINDOW_ID: WidgetId = WidgetId(110);
const FALLBACK_TITLE_BAR_H: f32 = 30.0;

const OK_ID: WidgetId = WidgetId(100);
const CANCEL_ID: WidgetId = WidgetId(101);





pub struct ServerListWindow {
    pub servers: Vec<ServerInfo>,
    pub selected_index: Option<usize>,
}

impl ServerListWindow {
    pub fn new(servers: Vec<ServerInfo>) -> Self {
        let selected_index = if servers.is_empty() { None } else { Some(0) };
        Self {
            servers,
            selected_index,
        }
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();

        if ui.ctx.key_down
            && let Some(idx) = self.selected_index
            && idx + 1 < self.servers.len()
        {
            self.selected_index = Some(idx + 1);
        }
        if ui.ctx.key_up
            && let Some(idx) = self.selected_index
            && idx > 0
        {
            self.selected_index = Some(idx - 1);
        }

        self.build_fallback(ui, &mut events);

        if ui.ctx.key_enter && self.selected_index.is_some() {
            events.push(GameEvent::RequestSelectServer {
                index: self.selected_index.unwrap(),
            });
        }
        if ui.ctx.key_escape {
            events.push(GameEvent::BackToLogin);
        }

        events
    }

    fn build_fallback(&mut self, ui: &mut UiFrame, events: &mut Vec<GameEvent>) {
        let row_h = ROW_H;
        let btn_w = FALLBACK_BTN_W;
        let btn_h = FALLBACK_BTN_H;
        let win_w = FALLBACK_WIN_W;
        let list_h = self.servers.len() as f32 * row_h;
        let padding = 8.0;
        let title_h = 30.0;
        let win_h = title_h + list_h + padding + btn_h + padding;
        let win = ui.window_account(SERVER_LIST_WINDOW_ID, win_w, win_h, FALLBACK_TITLE_BAR_H);

        let (v, i) = draw::quad_vertices(win.x, win.y, win.w, win_h, [0.08, 0.08, 0.12, 0.95]);
        ui.draw_calls.push(DrawCall {
            vertices: v.to_vec(),
            indices: i.to_vec(),
            texture: TextureRef::White,
        });

        let border_color = [0.4, 0.4, 0.5, 1.0];
        let b = 1.0;
        for (bx, by, bw, bh) in [
            (win.x, win.y, win.w, b),
            (win.x, win.y + win_h - b, win.w, b),
            (win.x, win.y, b, win_h),
            (win.x + win.w - b, win.y, b, win_h),
        ] {
            let (v, i) = draw::quad_vertices(bx, by, bw, bh, border_color);
            ui.draw_calls.push(DrawCall {
                vertices: v.to_vec(),
                indices: i.to_vec(),
                texture: TextureRef::White,
            });
        }

        let title = "Select Server";
        let title_w = ui.atlas.measure_text(title);
        let title_x = win.x + (win.w - title_w) / 2.0;
        let title_y = win.y + padding + ui.atlas.line_height;
        ui.text(title_x, title_y, title, [1.0, 1.0, 1.0, 1.0]);

        let list_y = win.y + title_h;
        for (idx, server) in self.servers.iter().enumerate() {
            let row_y = list_y + idx as f32 * row_h;
            let row_rect = Rect::new(win.x + padding, row_y, win.w - padding * 2.0, row_h);
            let row = ui.interact(
                WidgetId(SERVER_LIST_WINDOW_ID.0 + 10 + idx as u32),
                row_rect,
            );
            if row.hovered() {
                ui.any_interactive_hovered = true;
            }
            if row.clicked() {
                self.selected_index = Some(idx);
            }

            let bg_color = if self.selected_index == Some(idx) {
                [0.2, 0.2, 0.4, 1.0]
            } else if row.hovered() {
                [0.15, 0.15, 0.25, 1.0]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            };
            if bg_color[3] > 0.0 {
                let (v, i) =
                    draw::quad_vertices(row_rect.x, row_rect.y, row_rect.w, row_rect.h, bg_color);
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: i.to_vec(),
                    texture: TextureRef::White,
                });
            }

            let text_y = row_y + (row_h + ui.atlas.line_height) / 2.0 - (2.0);
            ui.text(
                win.x + padding + (4.0),
                text_y,
                &server.name,
                [1.0, 1.0, 1.0, 1.0],
            );

            let count = format!("{}", server.user_count);
            let count_w = ui.atlas.measure_text(&count);
            ui.text(
                win.x + win.w - padding - (4.0) - count_w,
                text_y,
                &count,
                [0.7, 0.7, 0.7, 1.0],
            );
        }

        let btn_y = list_y + list_h + padding;
        let btn_spacing = 8.0;
        let total_btn_w = btn_w * 2.0 + btn_spacing;
        let btn_start_x = win.x + (win.w - total_btn_w) / 2.0;

        let ok_rect = Rect::new(btn_start_x, btn_y, btn_w, btn_h);
        let cancel_rect = Rect::new(btn_start_x + btn_w + btn_spacing, btn_y, btn_w, btn_h);
        let ok = ui.button(OK_ID, ok_rect, &rust_i18n::t!("common.ok"));
        let cancel = ui.button(CANCEL_ID, cancel_rect, &rust_i18n::t!("common.cancel"));

        if ok.clicked() && self.selected_index.is_some() {
            events.push(GameEvent::RequestSelectServer {
                index: self.selected_index.unwrap(),
            });
        }
        if cancel.clicked() {
            events.push(GameEvent::BackToLogin);
        }
    }
}

impl Window for ServerListWindow {
    fn window_size(&self) -> (f32, f32) {
        let win_h = FALLBACK_TITLE_BAR_H
            + self.servers.len() as f32 * ROW_H
            + 8.0
            + FALLBACK_BTN_H
            + 8.0;
        (FALLBACK_WIN_W, win_h)
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    fn make_servers() -> Vec<ServerInfo> {
        vec![
            ServerInfo {
                ip: 0x0100007F,
                port: 6121,
                name: "Loki".into(),
                user_count: 42,
            },
            ServerInfo {
                ip: 0x0100007F,
                port: 6122,
                name: "Thor".into(),
                user_count: 17,
            },
            ServerInfo {
                ip: 0x0100007F,
                port: 6123,
                name: "Odin".into(),
                user_count: 5,
            },
        ]
    }

    #[test]
    fn arrow_keys_navigate_selection() {
        let mut win = ServerListWindow::new(make_servers());
        let mut state = StateCache::new();
        assert_eq!(win.selected_index, Some(0));

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_down = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(1));

        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(2));

        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(2));

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_up = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(1));

        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(0));

        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(0));
    }

    #[test]
    fn enter_with_selection_emits_request_select_server() {
        let mut win = ServerListWindow::new(make_servers());
        let mut state = StateCache::new();
        win.selected_index = Some(1);

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestSelectServer { index: 1 }))
        );
    }

    #[test]
    fn escape_emits_back_to_login() {
        let mut win = ServerListWindow::new(make_servers());
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_escape = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(events.iter().any(|e| matches!(e, GameEvent::BackToLogin)));
    }
}
