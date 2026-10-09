use crate::Window;
use crate::helper::window_chrome::{draw_container, draw_titlebar, label_color, text_color};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;




#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginFocus {
    Username,
    Password,
}

pub struct LoginWindow {
    pub username: TextInput,
    pub password: TextInput,
    pub keep_id: bool,
    pub focus: LoginFocus,
    win_size: (f32, f32),
}

const FALLBACK_WIN_W: f32 = 280.0;
const FALLBACK_WIN_H: f32 = 120.0;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;

const FIELD_X: f32 = 91.0;
const FIELD_RIGHT_MARGIN: f32 = 62.0;
const FIELD_H: f32 = 18.0;
const USERNAME_Y: f32 = 29.0;
const PASSWORD_Y: f32 = 61.0;
const CONNECT_BTN_RIGHT: f32 = 50.0;
const EXIT_BTN_RIGHT: f32 = 5.0;
const BTN_BOTTOM: f32 = 4.0;


pub const LOGIN_WINDOW_ID: WidgetId = WidgetId(10);
const TITLE_BAR_H: f32 = 25.0;

pub const USERNAME_ID: WidgetId = WidgetId(0);
pub const PASSWORD_ID: WidgetId = WidgetId(1);
const CONNECT_ID: WidgetId = WidgetId(2);
const EXIT_ID: WidgetId = WidgetId(3);
const KEEP_ID: WidgetId = WidgetId(4);

const FALLBACK_KEEP_W: f32 = 10.0;
const FALLBACK_KEEP_H: f32 = 10.0;
const LABEL_X: f32 = 12.0;

impl Default for LoginWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginWindow {
    pub fn new() -> Self {
        Self {
            username: TextInput::new(23, false),
            password: TextInput::new(23, true),
            keep_id: false,
            focus: LoginFocus::Username,
            win_size: (FALLBACK_WIN_W, FALLBACK_WIN_H),
        }
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let (win_w, win_h) = ((FALLBACK_WIN_W), (FALLBACK_WIN_H));
        let (btn_w, btn_h) = ((FALLBACK_BTN_W), (FALLBACK_BTN_H));
        let field_w = win_w - (FIELD_X) - (FIELD_RIGHT_MARGIN);
        let win = ui.window_account(LOGIN_WINDOW_ID, win_w, win_h, TITLE_BAR_H);

        if ui.ctx.key_tab {
            self.focus = match self.focus {
                LoginFocus::Username => LoginFocus::Password,
                LoginFocus::Password => LoginFocus::Username,
            };
            let focus_id = match self.focus {
                LoginFocus::Username => USERNAME_ID,
                LoginFocus::Password => PASSWORD_ID,
            };
            ui.set_focus(focus_id);
            match self.focus {
                LoginFocus::Username => self.username.move_cursor_to_end(),
                LoginFocus::Password => self.password.move_cursor_to_end(),
            }
        }

        draw_titlebar(ui, win.x, win.y, win.w, TITLE_BAR_H);
        draw_container(ui, win.x, win.y + TITLE_BAR_H, win.w, win.h - TITLE_BAR_H);
        let title = rust_i18n::t!("login.title");
        let title_w = ui.atlas.measure_text(&title);
        ui.text(
            win.x + (win.w - title_w) / 2.0,
            win.y + (TITLE_BAR_H + ui.atlas.line_height) / 2.0 - 3.0,
            &title,
            text_color(),
        );
        for (label, y) in [
            (rust_i18n::t!("login.account"), USERNAME_Y),
            (rust_i18n::t!("login.password"), PASSWORD_Y),
        ] {
            ui.text(win.x + LABEL_X, win.y + y + 13.0, &label, label_color());
        }

let input_bg = TextInputBg::Default;
        let username_rect = Rect::new(win.x + (FIELD_X), win.y + (USERNAME_Y), field_w, FIELD_H);
        let password_rect = Rect::new(win.x + (FIELD_X), win.y + (PASSWORD_Y), field_w, FIELD_H);
        ui.text_input(USERNAME_ID, username_rect, &mut self.username, input_bg);
        ui.text_input(PASSWORD_ID, password_rect, &mut self.password, input_bg);

        match ui.focused() {
            Some(id) if id == USERNAME_ID => self.focus = LoginFocus::Username,
            Some(id) if id == PASSWORD_ID => self.focus = LoginFocus::Password,
            _ => {}
        }

        let btn_y = win.y + win_h - (BTN_BOTTOM) - btn_h;
        let connect_rect = Rect::new(
            win.x + win_w - (CONNECT_BTN_RIGHT) - btn_w,
            btn_y,
            btn_w,
            btn_h,
        );
        let exit_rect = Rect::new(
            win.x + win_w - (EXIT_BTN_RIGHT) - btn_w,
            btn_y,
            btn_w,
            btn_h,
        );
        let connect = ui.button(CONNECT_ID, connect_rect, &rust_i18n::t!("login.connect"));
        let exit = ui.button(EXIT_ID, exit_rect, &rust_i18n::t!("login.exit"));

        let (keep_w, keep_h) = (FALLBACK_KEEP_W, FALLBACK_KEEP_H);
        let keep_rect = Rect::new(
            win.x + FIELD_X,
            btn_y + (btn_h - keep_h) / 2.0,
            keep_w,
            keep_h,
        );
        ui.checkbox(KEEP_ID, keep_rect, &mut self.keep_id);
        ui.text(
            keep_rect.x + keep_w + 6.0,
            btn_y + 14.0,
            &rust_i18n::t!("login.save_id"),
            label_color(),
        );

        let submit = ui.enter_pressed() || connect.clicked();
        if submit && !self.username.text.is_empty() && !self.password.text.is_empty() {
            events.push(GameEvent::RequestLogin {
                username: self.username.text.clone(),
                password: self.password.text.clone(),
            });
        }

        if exit.clicked() {
            events.push(GameEvent::Disconnected("User exit".to_string()));
        }

        events
    }
}

impl Window for LoginWindow {
    fn window_size(&self) -> (f32, f32) {
        (FALLBACK_WIN_W, FALLBACK_WIN_H)
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::TestFrame;

    fn make_ctx() -> UiContext {
        UiContext::new(800.0, 600.0)
    }

    #[test]
    fn tab_moves_caret_to_end_of_prefilled_field() {
        let mut login = LoginWindow::new();
        let mut state = StateCache::new();
        login.username.text = "admin".to_string();
        login.username.cursor_pos = 0;

        let mut ctx = make_ctx();
        ctx.key_tab = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui);
        assert_eq!(login.focus, LoginFocus::Password);

        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui);
        assert_eq!(login.focus, LoginFocus::Username);
        assert_eq!(login.username.cursor_pos, 5);
    }

    #[test]
    fn enter_with_credentials_emits_request_login() {
        let mut login = LoginWindow::new();
        let mut state = StateCache::new();
        login.username.text = "admin".to_string();
        login.password.text = "pass123".to_string();

        let mut ctx = make_ctx();
        ctx.key_enter = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        let events = login.build(&mut ui);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GameEvent::RequestLogin { username, password } => {
                assert_eq!(username, "admin");
                assert_eq!(password, "pass123");
            }
            _ => panic!("expected RequestLogin"),
        }
    }

    #[test]
    fn enter_with_empty_fields_does_nothing() {
        let mut login = LoginWindow::new();
        let mut state = StateCache::new();

        let mut ctx = make_ctx();
        ctx.key_enter = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        let events = login.build(&mut ui);
        assert!(events.is_empty());
    }

    #[test]
    fn clicking_keep_checkbox_toggles_state() {
        let mut login = LoginWindow::new();
        let mut state = StateCache::new();
        assert!(!login.keep_id);

        let win_x = (800.0 - FALLBACK_WIN_W) / 2.0;
        let win_y = (600.0 - FALLBACK_WIN_H) / 2.0;
        let keep_x = win_x + FIELD_X;
        let btn_y = win_y + FALLBACK_WIN_H - BTN_BOTTOM - FALLBACK_BTN_H;
        let keep_y = btn_y + (FALLBACK_BTN_H - FALLBACK_KEEP_H) / 2.0;

        let mut ctx = make_ctx();
        ctx.mouse_x = keep_x + FALLBACK_KEEP_W / 2.0;
        ctx.mouse_y = keep_y + FALLBACK_KEEP_H / 2.0;
        ctx.mouse_clicked = true;

        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui);
        assert!(login.keep_id);
    }
}
