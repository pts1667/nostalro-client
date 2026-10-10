use crate::Window;
use crate::account::server_list_panel::{self, PanelSpec, Row};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{UiFrame, WidgetId};





pub const LOGIN_SERVER_LIST_WINDOW_ID: WidgetId = WidgetId(4400);

const OK_ID: WidgetId = WidgetId(4401);
const CANCEL_ID: WidgetId = WidgetId(4402);





pub struct LoginServerEntry {
    pub name: String,
    pub detail: String,
}

pub struct LoginServerListWindow {
    pub servers: Vec<LoginServerEntry>,
    pub selected_index: Option<usize>,
}

impl LoginServerListWindow {
    pub fn new(servers: Vec<LoginServerEntry>) -> Self {
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
            events.push(GameEvent::SelectLoginServer {
                index: self.selected_index.unwrap(),
            });
        }
        if ui.ctx.key_escape {
            events.push(GameEvent::Disconnected("User exit".to_string()));
        }

        events
    }

    fn build_fallback(&mut self, ui: &mut UiFrame, events: &mut Vec<GameEvent>) {
        let rows: Vec<Row> = self
            .servers
            .iter()
            .map(|entry| Row {
                name: &entry.name,
                detail: &entry.detail,
            })
            .collect();
        let out = server_list_panel::build(
            ui,
            PanelSpec {
                window: LOGIN_SERVER_LIST_WINDOW_ID,
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
            events.push(GameEvent::SelectLoginServer { index });
        }
        if out.cancel {
            events.push(GameEvent::Disconnected("User exit".to_string()));
        }
    }
}

impl Window for LoginServerListWindow {
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

    fn make_servers() -> Vec<LoginServerEntry> {
        vec![
            LoginServerEntry {
                name: "Local".into(),
                detail: "127.0.0.1:6900".into(),
            },
            LoginServerEntry {
                name: "Live".into(),
                detail: "live.example.com:6900".into(),
            },
        ]
    }

    #[test]
    fn arrow_keys_navigate_and_enter_selects() {
        let mut win = LoginServerListWindow::new(make_servers());
        let mut state = StateCache::new();
        assert_eq!(win.selected_index, Some(0));

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_down = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        win.build(&mut ui);
        assert_eq!(win.selected_index, Some(1));

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::SelectLoginServer { index: 1 }))
        );
    }

    #[test]
    fn escape_exits() {
        let mut win = LoginServerListWindow::new(make_servers());
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_escape = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::Disconnected(_)))
        );
    }
}
