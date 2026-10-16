//! Asks the selected login server how it takes logins (`CA_SERVER_INFO`) and
//! registers accounts on it (`CA_CREATE_ACCOUNT`), on the login connection
//! before any `CA_LOGIN`.

use crate::App;
use ragnarok_game::app_state::AppState;
use ragnarok_network::NetworkCommand;
use ragnarok_network::account::{self, CreateAccountError};
use ragnarok_ui_component::account::login_window::LoginMode;
use std::time::{Duration, Instant};

/// How long the server has to answer `CA_SERVER_INFO`.
const TIMEOUT: Duration = Duration::from_secs(5);
/// Wait before asking again a server that did not answer.
const RETRY: Duration = Duration::from_secs(10);

/// What the login window opened the login connection for.
enum Pending {
    Info { since: Instant },
    Create { username: String, password: String },
}

pub(crate) struct LoginServerLink {
    /// The server the login window's mode came from.
    known: Option<String>,
    /// The server asked last and when.
    asked: Option<(String, Instant)>,
    pending: Option<Pending>,
}

impl LoginServerLink {
    pub(crate) fn new() -> Self {
        Self { known: None, asked: None, pending: None }
    }

    /// The login takes the connection over; answers still on their way are
    /// applied, but a failure no longer belongs to this link.
    pub(crate) fn release(&mut self) {
        self.pending = None;
    }

    /// Whether `addr` should be asked now: its mode is unknown, the connection
    /// is free, and a failed attempt is old enough.
    fn wants(&self, addr: &str) -> bool {
        if self.pending.is_some() || self.known.as_deref() == Some(addr) {
            return false;
        }
        match &self.asked {
            Some((asked, at)) if asked == addr => at.elapsed() >= RETRY,
            _ => true,
        }
    }

    fn info_timed_out(&self) -> bool {
        matches!(self.pending, Some(Pending::Info { since }) if since.elapsed() >= TIMEOUT)
    }
}

/// The mode the login window shows for a `LoginServerInfo` answer.
fn login_mode(accounts_required: bool, account_creation: bool) -> LoginMode {
    if accounts_required {
        LoginMode::Accounts { registration: account_creation }
    } else {
        LoginMode::NoAccount
    }
}

fn create_account_message(error: CreateAccountError) -> String {
    match error {
        CreateAccountError::Disabled => "This server does not create accounts".to_string(),
        CreateAccountError::InvalidName => {
            "Invalid account name: use letters, digits and _, at the server's minimum length, \
             and do not end it in _M or _F"
                .to_string()
        }
        CreateAccountError::InvalidPassword => "The password is too short".to_string(),
        CreateAccountError::InvalidSex => "Invalid sex".to_string(),
        CreateAccountError::NameTaken => "That account name is taken".to_string(),
        CreateAccountError::Limited => {
            "Too many accounts were created lately, try again later".to_string()
        }
        CreateAccountError::Failed => "The server could not store the account".to_string(),
        CreateAccountError::Banned => "This address is banned".to_string(),
        CreateAccountError::Unknown(code) => format!("Account creation failed (code {code})"),
    }
}

impl App {
    /// `host:port` of the selected login server.
    pub(crate) fn login_server_addr(&self) -> Option<String> {
        let server = self.config.login_servers.get(self.selected_login_server)?;
        Some(format!("{}:{}", server.host, server.port))
    }

    /// Opens the login connection to `addr`, nothing sent yet.
    fn connect_login_server(&mut self, addr: String) {
        self.channel.send_cmd(NetworkCommand::Connect {
            addr,
            expect_aid: false,
        });
    }

    /// Asks the selected login server for its login mode while the login
    /// screen is up, and gives up on an answer that is too late.
    pub(crate) fn poll_login_server(&mut self) {
        let addr = self.login_server_addr();
        if self.login_server.known.is_some() && self.login_server.known != addr {
            self.login_server.known = None;
            self.login_window.set_mode(LoginMode::Accounts { registration: false });
        }
        if self.login_server.info_timed_out() {
            tracing::warn!("The login server did not say how it takes logins");
            self.login_server.pending = None;
            self.channel.send_cmd(NetworkCommand::Disconnect);
        }
        if self.game.session.app_state == AppState::Login
            && let Some(addr) = addr
            && self.login_server.wants(&addr)
        {
            self.login_server.asked = Some((addr.clone(), Instant::now()));
            self.login_server.pending = Some(Pending::Info { since: Instant::now() });
            self.connect_login_server(addr);
            self.channel.send_packet(account::build_server_info_request());
        }
    }

    pub(crate) fn handle_login_server_info(
        &mut self,
        accounts_required: bool,
        account_creation: bool,
        packetver: u32,
    ) {
        let Some(addr) = self.login_server_addr() else {
            return;
        };
        tracing::info!(
            "Login server {addr}: accounts {}, account creation {}, packetver {packetver}",
            if accounts_required { "required" } else { "skipped" },
            if account_creation { "on" } else { "off" },
        );
        if packetver != self.active_packetver {
            tracing::warn!(
                "Login server {addr} runs packetver {packetver} but the config sets {}",
                self.active_packetver
            );
        }
        self.login_window
            .set_mode(login_mode(accounts_required, account_creation));
        self.login_server.known = Some(addr);
        if matches!(self.login_server.pending, Some(Pending::Info { .. })) {
            self.login_server.pending = None;
            self.channel.send_cmd(NetworkCommand::Disconnect);
        }
    }

    /// Takes a disconnection the login window caused, so it shows no
    /// "Disconnected" box: a server that cannot be asked keeps the plain login,
    /// and a failed registration says why. False for any other.
    pub(crate) fn login_server_disconnected(&mut self, reason: &str) -> bool {
        match self.login_server.pending.take() {
            Some(Pending::Info { .. }) => {
                tracing::warn!("Cannot ask the login server how it takes logins: {reason}");
                true
            }
            Some(Pending::Create { .. }) => {
                let message = format!("Cannot reach the login server: {reason}");
                self.account_dialog.show(&message, false, |_| {});
                true
            }
            None => false,
        }
    }

    /// Registers an account on the selected login server; it logs in with it once created.
    pub(crate) fn request_create_account(&mut self, username: String, password: String, sex: char) {
        let Some(addr) = self.login_server_addr() else {
            return;
        };
        self.account_dialog.show_message("Please wait...");
        let packet = account::build_create_account_packet(&username, &password, sex);
        self.login_server.pending = Some(Pending::Create { username, password });
        self.connect_login_server(addr);
        self.channel.send_packet(packet);
    }

    pub(crate) fn handle_account_create_result(&mut self, code: u8, account_id: u32) {
        let Some(Pending::Create { username, password }) = self.login_server.pending.take() else {
            return;
        };
        match CreateAccountError::from_code(code) {
            None => {
                tracing::info!("Created account {username} ({account_id})");
                self.login_window.show_login();
                self.start_login(username, password);
            }
            Some(error) => {
                self.channel.send_cmd(NetworkCommand::Disconnect);
                if error == CreateAccountError::Disabled {
                    self.login_window.set_mode(LoginMode::Accounts { registration: false });
                }
                self.account_dialog.show(&create_account_message(error), false, |_| {});
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_info_picks_the_login_mode() {
        assert_eq!(login_mode(true, false), LoginMode::Accounts { registration: false });
        assert_eq!(login_mode(true, true), LoginMode::Accounts { registration: true });
        assert_eq!(login_mode(false, true), LoginMode::NoAccount);
    }

    #[test]
    fn a_server_is_asked_once_until_it_fails_then_after_a_pause() {
        let mut link = LoginServerLink::new();
        assert!(link.wants("a:1"));
        link.pending = Some(Pending::Info { since: Instant::now() });
        link.asked = Some(("a:1".to_string(), Instant::now()));
        assert!(!link.wants("a:1"));
        assert!(!link.info_timed_out());
        link.pending = None;
        assert!(!link.wants("a:1"), "a failure waits before asking again");
        assert!(link.wants("b:2"), "another server is asked at once");
        link.asked = Some(("a:1".to_string(), Instant::now() - RETRY));
        assert!(link.wants("a:1"));
        link.known = Some("a:1".to_string());
        assert!(!link.wants("a:1"));
    }

    #[test]
    fn a_late_answer_times_out_and_a_registration_holds_the_connection() {
        let mut link = LoginServerLink::new();
        link.pending = Some(Pending::Info { since: Instant::now() - TIMEOUT });
        assert!(link.info_timed_out());
        link.pending = Some(Pending::Create { username: "a".into(), password: "b".into() });
        assert!(!link.info_timed_out());
        assert!(!link.wants("a:1"));
        link.release();
        assert!(link.wants("a:1"));
    }
}
