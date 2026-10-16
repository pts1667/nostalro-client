//! The login server's own packets for custom clients (rust-ro
//! `docs/custom-login-packets.md`): asking how logins work there, and creating
//! an account without logging in. They travel on the login connection; the
//! generated parser does not know them, so [`frame_len`] frames the answers
//! and [`dispatch`] turns them into events.

use ragnarok_game::event::GameEvent;

const CA_CREATE_ACCOUNT: u16 = 0xC3E0;
const AC_CREATE_ACCOUNT_RESULT: u16 = 0xC3E1;
const CA_SERVER_INFO: u16 = 0xC3E2;
const AC_SERVER_INFO: u16 = 0xC3E3;
const CREATE_ACCOUNT_RESULT_LEN: usize = 7;
const SERVER_INFO_LEN: usize = 8;
const NAME_FIELD: usize = 24;

/// Why the server refused to create an account, by its result code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateAccountError {
    Disabled,
    InvalidName,
    InvalidPassword,
    InvalidSex,
    NameTaken,
    Limited,
    Failed,
    Banned,
    Unknown(u8),
}

impl CreateAccountError {
    /// `None` for `0`, a success.
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => return None,
            1 => Self::Disabled,
            2 => Self::InvalidName,
            3 => Self::InvalidPassword,
            4 => Self::InvalidSex,
            5 => Self::NameTaken,
            6 => Self::Limited,
            7 => Self::Failed,
            8 => Self::Banned,
            other => Self::Unknown(other),
        })
    }
}

/// Wire length of a custom answer from the login server, by packet id.
pub fn frame_len(id: u16) -> Option<usize> {
    match id {
        AC_CREATE_ACCOUNT_RESULT => Some(CREATE_ACCOUNT_RESULT_LEN),
        AC_SERVER_INFO => Some(SERVER_INFO_LEN),
        _ => None,
    }
}

/// The event for a custom answer framed by [`frame_len`].
pub fn dispatch(raw: &[u8]) -> Option<GameEvent> {
    if raw.len() < 2 {
        return None;
    }
    match u16::from_le_bytes([raw[0], raw[1]]) {
        AC_SERVER_INFO if raw.len() >= SERVER_INFO_LEN => Some(GameEvent::LoginServerInfo {
            accounts_required: raw[2] != 0,
            account_creation: raw[3] != 0,
            packetver: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        }),
        AC_CREATE_ACCOUNT_RESULT if raw.len() >= CREATE_ACCOUNT_RESULT_LEN => {
            Some(GameEvent::AccountCreateResult {
                code: raw[2],
                account_id: u32::from_le_bytes([raw[3], raw[4], raw[5], raw[6]]),
            })
        }
        _ => None,
    }
}

pub fn build_server_info_request() -> Vec<u8> {
    CA_SERVER_INFO.to_le_bytes().to_vec()
}

/// `sex` is `'M'` or `'F'`. Name and password are cut to 23 bytes, as in `CA_LOGIN`.
pub fn build_create_account_packet(username: &str, password: &str, sex: char) -> Vec<u8> {
    let mut packet = CA_CREATE_ACCOUNT.to_le_bytes().to_vec();
    for field in [username, password] {
        let bytes = field.as_bytes();
        let mut padded = [0u8; NAME_FIELD];
        let len = bytes.len().min(NAME_FIELD - 1);
        padded[..len].copy_from_slice(&bytes[..len]);
        packet.extend_from_slice(&padded);
    }
    packet.push(sex as u8);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_info_reads_the_account_mode_and_packet_version() {
        assert_eq!(build_server_info_request(), vec![0xE2, 0xC3]);
        let mut raw = vec![0xE3, 0xC3, 1, 1];
        raw.extend_from_slice(&20120307u32.to_le_bytes());
        assert_eq!(frame_len(0xC3E3), Some(raw.len()));
        match dispatch(&raw) {
            Some(GameEvent::LoginServerInfo { accounts_required, account_creation, packetver }) => {
                assert!(accounts_required && account_creation);
                assert_eq!(packetver, 20120307);
            }
            _ => panic!("expected LoginServerInfo"),
        }
        match dispatch(&[0xE3, 0xC3, 0, 0, 0, 0, 0, 0]) {
            Some(GameEvent::LoginServerInfo { accounts_required, account_creation, .. }) => {
                assert!(!accounts_required && !account_creation);
            }
            _ => panic!("expected LoginServerInfo"),
        }
        assert!(dispatch(&[0xE3, 0xC3, 1]).is_none());
    }

    #[test]
    fn create_account_sends_the_padded_fields_and_reads_the_result() {
        let request = build_create_account_packet("maker", "secret", 'F');
        assert_eq!(request.len(), 51);
        assert_eq!(&request[..2], &[0xE0, 0xC3]);
        assert_eq!(&request[2..7], b"maker");
        assert!(request[7..26].iter().all(|b| *b == 0));
        assert_eq!(&request[26..32], b"secret");
        assert!(request[32..50].iter().all(|b| *b == 0));
        assert_eq!(request[50], b'F');

        assert_eq!(frame_len(0xC3E1), Some(7));
        match dispatch(&[0xE1, 0xC3, 0, 0x81, 0x84, 0x1E, 0x00]) {
            Some(GameEvent::AccountCreateResult { code, account_id }) => {
                assert_eq!((code, account_id), (0, 2_000_001));
            }
            _ => panic!("expected AccountCreateResult"),
        }
        assert_eq!(CreateAccountError::from_code(0), None);
        assert_eq!(CreateAccountError::from_code(5), Some(CreateAccountError::NameTaken));
        assert_eq!(CreateAccountError::from_code(42), Some(CreateAccountError::Unknown(42)));
    }

    #[test]
    fn long_fields_keep_a_terminating_nul() {
        let packet = build_create_account_packet(&"n".repeat(30), &"p".repeat(30), 'M');
        assert_eq!(packet.len(), 51);
        assert_eq!(packet[25], 0);
        assert_eq!(packet[49], 0);
    }

    #[test]
    fn requests_are_not_framed_as_answers() {
        assert_eq!(frame_len(0xC3E0), None);
        assert_eq!(frame_len(0xC3E2), None);
        assert_eq!(frame_len(0x0A28), None);
    }
}
