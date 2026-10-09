use crate::Window;
use crate::account::server_list_panel::{self, PanelSpec, Row};
use ragnarok_game::event::{GameEvent, ServerInfo};
use ragnarok_ui::frame::{UiFrame, WidgetId};





pub const SERVER_LIST_WINDOW_ID: WidgetId = WidgetId(110);

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
        let details: Vec<String> = self
            .servers
            .iter()
            .map(|server| server.user_count.to_string())
            .collect();
        let rows: Vec<Row> = self
            .servers
            .iter()
            .zip(&details)
            .map(|(server, detail)| Row {
                name: &server.name,
                detail,
            })
            .collect();
        let out = server_list_panel::build(
            ui,
            PanelSpec {
                window: SERVER_LIST_WINDOW_ID,
                ok: OK_ID,
                cancel: CANCEL_ID,
                title: &rust_i18n::t!("server_list.title"),
            },
            &rows,
            &mut self.selected_index,
        );
        if out.ok
            && let Some(index) = self.selected_index
        {
            events.push(GameEvent::RequestSelectServer { index });
        }
        if out.cancel {
            events.push(GameEvent::BackToLogin);
        }
    }
}

impl Window for ServerListWindow {
    fn window_size(&self) -> (f32, f32) {
        server_list_panel::size(self.servers.len())
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
