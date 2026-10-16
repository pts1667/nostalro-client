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
    Confirm,
}

/// How the login server takes this client, as it answers `CA_SERVER_INFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
    /// Account name and password. `registration` adds the Register screen.
    Accounts { registration: bool },
    /// Any name logs in without a password; an unknown one gets an account.
    NoAccount,
}

pub struct LoginWindow {
    pub username: TextInput,
    pub password: TextInput,
    /// The password typed again on the Register screen.
    pub confirm: TextInput,
    pub keep_id: bool,
    pub focus: LoginFocus,
    mode: LoginMode,
    /// Showing the Register screen rather than the login.
    pub registering: bool,
    /// The sex of an account created here, `M` when false.
    pub female: bool,
    /// Why the Register screen did not send, shown above its buttons.
    problem: Option<String>,
}

const FALLBACK_WIN_W: f32 = 280.0;
const FALLBACK_WIN_H: f32 = 120.0;
const REGISTER_WIN_H: f32 = 172.0;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;
const REGISTER_BTN_W: f32 = 60.0;

const FIELD_X: f32 = 91.0;
const FIELD_RIGHT_MARGIN: f32 = 62.0;
const FIELD_H: f32 = 18.0;
const USERNAME_Y: f32 = 29.0;
const PASSWORD_Y: f32 = 61.0;
/// Rows of the Register screen: account, password, confirmation, sex.
const REGISTER_ROWS: [f32; 4] = [29.0, 55.0, 81.0, 107.0];
const PROBLEM_Y: f32 = 129.0;
const CONNECT_BTN_RIGHT: f32 = 50.0;
const EXIT_BTN_RIGHT: f32 = 5.0;
const REGISTER_BTN_LEFT: f32 = 8.0;
const BTN_BOTTOM: f32 = 4.0;
const SEX_GAP: f32 = 60.0;


pub const LOGIN_WINDOW_ID: WidgetId = WidgetId(10);
const TITLE_BAR_H: f32 = 25.0;

pub const USERNAME_ID: WidgetId = WidgetId(0);
pub const PASSWORD_ID: WidgetId = WidgetId(1);
const CONNECT_ID: WidgetId = WidgetId(2);
const EXIT_ID: WidgetId = WidgetId(3);
const KEEP_ID: WidgetId = WidgetId(4);
pub const CONFIRM_ID: WidgetId = WidgetId(5);
const REGISTER_ID: WidgetId = WidgetId(6);
const MALE_ID: WidgetId = WidgetId(7);
const FEMALE_ID: WidgetId = WidgetId(8);

const FALLBACK_KEEP_W: f32 = 10.0;
const FALLBACK_KEEP_H: f32 = 10.0;
const LABEL_X: f32 = 12.0;
const MAX_ID_LEN: usize = 23;

impl Default for LoginWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// `name_M` / `name_F`, which the login reads as the sex of a new account.
fn has_sex_suffix(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 2
        && bytes[bytes.len() - 2] == b'_'
        && matches!(bytes[bytes.len() - 1].to_ascii_uppercase(), b'M' | b'F')
}

impl LoginWindow {
    pub fn new() -> Self {
        Self {
            username: TextInput::new(MAX_ID_LEN, false),
            password: TextInput::new(MAX_ID_LEN, true),
            confirm: TextInput::new(MAX_ID_LEN, true),
            keep_id: false,
            focus: LoginFocus::Username,
            mode: LoginMode::Accounts { registration: false },
            registering: false,
            female: false,
            problem: None,
        }
    }

    pub fn mode(&self) -> LoginMode {
        self.mode
    }

    /// Adopts the server's way of logging in, leaving a screen it lacks.
    pub fn set_mode(&mut self, mode: LoginMode) {
        self.mode = mode;
        if mode != (LoginMode::Accounts { registration: true }) {
            self.show_login();
        }
        if !self.fields().contains(&self.focus) {
            self.focus = LoginFocus::Username;
        }
    }

    /// Back from the Register screen to the login, keeping the name and password.
    pub fn show_login(&mut self) {
        self.registering = false;
        self.problem = None;
        self.confirm.text.clear();
        self.confirm.cursor_pos = 0;
        if self.focus == LoginFocus::Confirm {
            self.focus = LoginFocus::Password;
        }
    }

    fn show_register(&mut self) {
        self.registering = true;
        self.problem = None;
        self.focus = LoginFocus::Username;
    }

    /// The widget to focus for `focus`.
    pub fn focus_id(&self) -> WidgetId {
        if !self.fields().contains(&self.focus) {
            return USERNAME_ID;
        }
        match self.focus {
            LoginFocus::Username => USERNAME_ID,
            LoginFocus::Password => PASSWORD_ID,
            LoginFocus::Confirm => CONFIRM_ID,
        }
    }

    /// The text fields on show, in Tab order.
    fn fields(&self) -> &'static [LoginFocus] {
        if self.registering {
            &[LoginFocus::Username, LoginFocus::Password, LoginFocus::Confirm]
        } else if self.mode == LoginMode::NoAccount {
            &[LoginFocus::Username]
        } else {
            &[LoginFocus::Username, LoginFocus::Password]
        }
    }

    fn input(&mut self, focus: LoginFocus) -> &mut TextInput {
        match focus {
            LoginFocus::Username => &mut self.username,
            LoginFocus::Password => &mut self.password,
            LoginFocus::Confirm => &mut self.confirm,
        }
    }

    fn sex(&self) -> char {
        if self.female { 'F' } else { 'M' }
    }

    /// The login id of a password-less login: the name, with the chosen sex as
    /// a suffix for the account it may create, unless the name has one already
    /// or there is no room for it.
    fn no_account_id(&self) -> String {
        let name = &self.username.text;
        if has_sex_suffix(name) || name.len() + 2 > MAX_ID_LEN {
            name.clone()
        } else {
            format!("{name}_{}", self.sex())
        }
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let (win_w, win_h) = self.window_size();
        let (btn_w, btn_h) = ((FALLBACK_BTN_W), (FALLBACK_BTN_H));
        let field_w = win_w - (FIELD_X) - (FIELD_RIGHT_MARGIN);
        let win = ui.window_account(LOGIN_WINDOW_ID, win_w, win_h, TITLE_BAR_H);

        let fields = self.fields();
        if ui.ctx.key_tab && fields.len() > 1 {
            let at = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
            self.focus = fields[(at + 1) % fields.len()];
            ui.set_focus(self.focus_id());
            self.input(self.focus).move_cursor_to_end();
        }

        draw_titlebar(ui, win.x, win.y, win.w, TITLE_BAR_H);
        draw_container(ui, win.x, win.y + TITLE_BAR_H, win.w, win.h - TITLE_BAR_H);
        let title = if self.registering {
            rust_i18n::t!("login.register_title")
        } else {
            rust_i18n::t!("login.title")
        };
        let title_w = ui.atlas.measure_text(&title);
        ui.text(
            win.x + (win.w - title_w) / 2.0,
            win.y + (TITLE_BAR_H + ui.atlas.line_height) / 2.0 - 3.0,
            &title,
            text_color(),
        );

        let (rows, sex_y): (Vec<(LoginFocus, f32)>, Option<f32>) = if self.registering {
            let [account, password, confirm, sex] = REGISTER_ROWS;
            (
                vec![
                    (LoginFocus::Username, account),
                    (LoginFocus::Password, password),
                    (LoginFocus::Confirm, confirm),
                ],
                Some(sex),
            )
        } else if self.mode == LoginMode::NoAccount {
            (vec![(LoginFocus::Username, USERNAME_Y)], Some(PASSWORD_Y))
        } else {
            (
                vec![(LoginFocus::Username, USERNAME_Y), (LoginFocus::Password, PASSWORD_Y)],
                None,
            )
        };

let input_bg = TextInputBg::Default;
        for &(field, y) in &rows {
            let label = match field {
                LoginFocus::Username => rust_i18n::t!("login.account"),
                LoginFocus::Password => rust_i18n::t!("login.password"),
                LoginFocus::Confirm => rust_i18n::t!("login.confirm"),
            };
            ui.text(win.x + LABEL_X, win.y + y + 13.0, &label, label_color());
            let id = match field {
                LoginFocus::Username => USERNAME_ID,
                LoginFocus::Password => PASSWORD_ID,
                LoginFocus::Confirm => CONFIRM_ID,
            };
            let rect = Rect::new(win.x + (FIELD_X), win.y + y, field_w, FIELD_H);
            ui.text_input(id, rect, self.input(field), input_bg);
        }

        match ui.focused() {
            Some(id) if id == USERNAME_ID => self.focus = LoginFocus::Username,
            Some(id) if id == PASSWORD_ID => self.focus = LoginFocus::Password,
            Some(id) if id == CONFIRM_ID => self.focus = LoginFocus::Confirm,
            _ => {}
        }

        let (keep_w, keep_h) = (FALLBACK_KEEP_W, FALLBACK_KEEP_H);
        if let Some(y) = sex_y {
            ui.text(win.x + LABEL_X, win.y + y + 13.0, &rust_i18n::t!("login.sex"), label_color());
            let box_y = win.y + y + (FIELD_H - keep_h) / 2.0;
            for (i, (id, female, label)) in [
                (MALE_ID, false, rust_i18n::t!("login.male")),
                (FEMALE_ID, true, rust_i18n::t!("login.female")),
            ]
            .into_iter()
            .enumerate()
            {
                let x = win.x + FIELD_X + i as f32 * SEX_GAP;
                let mut checked = self.female == female;
                if ui.checkbox(id, Rect::new(x, box_y, keep_w, keep_h), &mut checked).clicked() {
                    self.female = female;
                }
                ui.text(x + keep_w + 6.0, win.y + y + 13.0, &label, label_color());
            }
        }

        if let Some(problem) = &self.problem {
            ui.text(win.x + LABEL_X, win.y + PROBLEM_Y + 13.0, problem, [0.8, 0.1, 0.1, 1.0]);
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

        if self.registering {
            let create = ui.button(CONNECT_ID, connect_rect, &rust_i18n::t!("login.create"));
            let back = ui.button(EXIT_ID, exit_rect, &rust_i18n::t!("login.back"));
            if ui.enter_pressed() || create.clicked() {
                events.extend(self.submit_registration());
            } else if back.clicked() {
                self.show_login();
            }
            return events;
        }

        let connect = ui.button(CONNECT_ID, connect_rect, &rust_i18n::t!("login.connect"));
        let exit = ui.button(EXIT_ID, exit_rect, &rust_i18n::t!("login.exit"));

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

        if self.mode == (LoginMode::Accounts { registration: true }) {
            let register_rect =
                Rect::new(win.x + REGISTER_BTN_LEFT, btn_y, REGISTER_BTN_W, btn_h);
            if ui.button(REGISTER_ID, register_rect, &rust_i18n::t!("login.register")).clicked() {
                self.show_register();
                ui.set_focus(USERNAME_ID);
                return events;
            }
        }

        let submit = ui.enter_pressed() || connect.clicked();
        if submit && !self.username.text.is_empty() {
            if self.mode == LoginMode::NoAccount {
                events.push(GameEvent::RequestLogin {
                    username: self.no_account_id(),
                    password: String::new(),
                });
            } else if !self.password.text.is_empty() {
                events.push(GameEvent::RequestLogin {
                    username: self.username.text.clone(),
                    password: self.password.text.clone(),
                });
            }
        }

        if exit.clicked() {
            events.push(GameEvent::Disconnected("User exit".to_string()));
        }

        events
    }

    fn submit_registration(&mut self) -> Option<GameEvent> {
        self.problem = if self.username.text.is_empty() {
            Some(rust_i18n::t!("login.name_missing").into_owned())
        } else if self.password.text.is_empty() {
            Some(rust_i18n::t!("login.password_missing").into_owned())
        } else if self.password.text != self.confirm.text {
            Some(rust_i18n::t!("login.passwords_differ").into_owned())
        } else {
            None
        };
        if self.problem.is_some() {
            return None;
        }
        Some(GameEvent::RequestCreateAccount {
            username: self.username.text.clone(),
            password: self.password.text.clone(),
            sex: self.sex(),
        })
    }
}

impl Window for LoginWindow {
    fn window_size(&self) -> (f32, f32) {
        let h = if self.registering { REGISTER_WIN_H } else { FALLBACK_WIN_H };
        (FALLBACK_WIN_W, h)
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

    fn press_enter(login: &mut LoginWindow) -> Vec<GameEvent> {
        let mut state = StateCache::new();
        let mut ctx = make_ctx();
        ctx.key_enter = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui)
    }

    /// Clicks the middle of `rect`, given relative to the centred window.
    fn click(login: &mut LoginWindow, x: f32, y: f32, w: f32, h: f32) -> Vec<GameEvent> {
        let (win_w, win_h) = login.window_size();
        let mut state = StateCache::new();
        let mut ctx = make_ctx();
        ctx.mouse_x = (800.0 - win_w) / 2.0 + x + w / 2.0;
        ctx.mouse_y = (600.0 - win_h) / 2.0 + y + h / 2.0;
        ctx.mouse_clicked = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui)
    }

    fn btn_y(login: &LoginWindow) -> f32 {
        login.window_size().1 - BTN_BOTTOM - FALLBACK_BTN_H
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
        login.username.text = "admin".to_string();
        login.password.text = "pass123".to_string();

        let events = press_enter(&mut login);
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
        assert!(press_enter(&mut login).is_empty());
    }

    #[test]
    fn clicking_keep_checkbox_toggles_state() {
        let mut login = LoginWindow::new();
        assert!(!login.keep_id);
        let keep_y = btn_y(&login) + (FALLBACK_BTN_H - FALLBACK_KEEP_H) / 2.0;
        click(&mut login, FIELD_X, keep_y, FALLBACK_KEEP_W, FALLBACK_KEEP_H);
        assert!(login.keep_id);
    }

    #[test]
    fn without_accounts_a_name_alone_logs_in_with_the_chosen_sex() {
        let mut login = LoginWindow::new();
        login.set_mode(LoginMode::NoAccount);
        login.username.text = "bot".to_string();

        let female_x = FIELD_X + SEX_GAP;
        let box_y = PASSWORD_Y + (FIELD_H - FALLBACK_KEEP_H) / 2.0;
        click(&mut login, female_x, box_y, FALLBACK_KEEP_W, FALLBACK_KEEP_H);
        assert!(login.female);

        match &press_enter(&mut login)[..] {
            [GameEvent::RequestLogin { username, password }] => {
                assert_eq!(username, "bot_F");
                assert!(password.is_empty());
            }
            other => panic!("expected one RequestLogin, got {other:?}"),
        }

        login.username.text = "bot_m".to_string();
        match &press_enter(&mut login)[..] {
            [GameEvent::RequestLogin { username, .. }] => assert_eq!(username, "bot_m"),
            other => panic!("expected one RequestLogin, got {other:?}"),
        }
    }

    #[test]
    fn without_accounts_tab_stays_on_the_name() {
        let mut login = LoginWindow::new();
        login.focus = LoginFocus::Password;
        login.set_mode(LoginMode::NoAccount);
        assert_eq!(login.focus, LoginFocus::Username);
        let mut state = StateCache::new();
        let mut ctx = make_ctx();
        ctx.key_tab = true;
        let mut ui = TestFrame::new()
            .focus(USERNAME_ID)
            .build(&mut ctx, &mut state);
        login.build(&mut ui);
        assert_eq!(login.focus, LoginFocus::Username);
    }

    #[test]
    fn register_button_shows_only_when_the_server_creates_accounts() {
        let mut login = LoginWindow::new();
        let y = btn_y(&login);
        click(&mut login, REGISTER_BTN_LEFT, y, REGISTER_BTN_W, FALLBACK_BTN_H);
        assert!(!login.registering);

        login.set_mode(LoginMode::Accounts { registration: true });
        click(&mut login, REGISTER_BTN_LEFT, y, REGISTER_BTN_W, FALLBACK_BTN_H);
        assert!(login.registering);
        assert_eq!(login.window_size().1, REGISTER_WIN_H);

        login.set_mode(LoginMode::Accounts { registration: false });
        assert!(!login.registering);
    }

    #[test]
    fn registration_checks_the_confirmation_then_asks_for_the_account() {
        let mut login = LoginWindow::new();
        login.set_mode(LoginMode::Accounts { registration: true });
        login.registering = true;
        login.username.text = "maker".to_string();
        login.password.text = "secret".to_string();
        login.confirm.text = "secreT".to_string();
        assert!(press_enter(&mut login).is_empty());
        assert!(login.problem.is_some());

        login.confirm.text = "secret".to_string();
        login.female = true;
        match &press_enter(&mut login)[..] {
            [GameEvent::RequestCreateAccount { username, password, sex }] => {
                assert_eq!((username.as_str(), password.as_str(), *sex), ("maker", "secret", 'F'));
            }
            other => panic!("expected one RequestCreateAccount, got {other:?}"),
        }
        assert!(login.problem.is_none());
    }

    #[test]
    fn back_leaves_the_register_screen_keeping_name_and_password() {
        let mut login = LoginWindow::new();
        login.set_mode(LoginMode::Accounts { registration: true });
        login.registering = true;
        login.username.text = "maker".to_string();
        login.password.text = "secret".to_string();
        login.confirm.text = "secret".to_string();
        let y = btn_y(&login);
        let back_x = FALLBACK_WIN_W - EXIT_BTN_RIGHT - FALLBACK_BTN_W;
        let events = click(&mut login, back_x, y, FALLBACK_BTN_W, FALLBACK_BTN_H);
        assert!(events.is_empty());
        assert!(!login.registering);
        assert!(login.confirm.text.is_empty());
        assert_eq!((login.username.text.as_str(), login.password.text.as_str()), ("maker", "secret"));
    }
}
