//! Talks to the server's forum (`/forum`, on the bot API port) for the forum
//! window. It signs in as the playing character with the account and password
//! of the game login, keeps the token it gets, and signs in again when the
//! character changes or the server no longer knows the token (it ends after a
//! day and on a server restart).
//!
//! Two threads share the token. One runs the window's requests one at a time,
//! so a slow server never stalls a frame. The other keeps the forum's WebSocket
//! (`/forum/ws`) open while a character is in game and passes on each reply to
//! that character's threads and comments, starting after the last one it
//! handled. Answers are collected by `poll`.

use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ragnarok_game::app_state::AppState;
use ragnarok_game::event::ForumRequest;
use serde_json::{Value, json};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{HandshakeError, Message, WebSocket};

use crate::App;
use crate::reference::{join_url, parse_answer};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a read of the WebSocket waits before the watcher checks whether
/// the character it watches changed.
const WATCH_TICK: Duration = Duration::from_millis(500);
/// Wait before connecting again after a failure, doubled up to the maximum.
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// A connection that held this long starts the waits over from the minimum.
const STABLE: Duration = Duration::from_secs(30);

/// Who the forum is signed in as: the game login and the character playing.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) account: String,
    pub(crate) password: String,
    pub(crate) character: String,
}

/// The character the WebSocket watches, and the `seq` of the last reply to it
/// already handled.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Watch {
    pub(crate) identity: Identity,
    pub(crate) after: u64,
}

struct Job {
    identity: Identity,
    request: ForumRequest,
}

pub(crate) enum ForumAnswer {
    /// The server accepted the sign-in; the name is the character's as it spells it.
    SignedIn(String),
    Reply(ForumRequest, Result<Value, String>),
    /// A `reply` event of the WebSocket: someone answered the watched character.
    Notice(Value),
}

pub(crate) struct ForumClient {
    jobs: Sender<Job>,
    watches: Sender<Option<Watch>>,
    answers: Receiver<ForumAnswer>,
    /// The character last handed to the watcher.
    watching: Option<Identity>,
}

/// A failed call. `Unauthorized` makes the worker sign in again.
#[derive(Debug, PartialEq)]
enum Failure {
    Unauthorized(String),
    Other(String),
}

impl Failure {
    fn message(self) -> String {
        match self {
            Failure::Unauthorized(message) | Failure::Other(message) => message,
        }
    }
}

struct Session {
    identity: Identity,
    token: String,
}

/// What the two threads share: the server, the token and the way back.
struct Shared {
    base_url: String,
    agent: ureq::Agent,
    session: Mutex<Option<Session>>,
    answers: Sender<ForumAnswer>,
}

impl Shared {
    fn session(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The token of `identity`, signing in when there is none. The lock is held
    /// while signing in, so the two threads never sign in at once.
    fn token(&self, identity: &Identity) -> Result<(String, bool), Failure> {
        let mut session = self.session();
        if let Some(current) = session.as_ref()
            && current.identity == *identity
        {
            return Ok((current.token.clone(), false));
        }
        *session = None;
        let (token, name) = sign_in(&self.agent, &self.base_url, identity)?;
        *session = Some(Session { identity: identity.clone(), token: token.clone() });
        let _ = self.answers.send(ForumAnswer::SignedIn(name));
        Ok((token, true))
    }

    /// Forgets `token` after the server refused it, unless the other thread
    /// already replaced it.
    fn forget(&self, token: &str) {
        let mut session = self.session();
        if session.as_ref().is_some_and(|s| s.token == token) {
            *session = None;
        }
    }
}

impl ForumClient {
    /// `base_url` is the forum server, for instance `http://127.0.0.1:6902`.
    pub(crate) fn spawn(base_url: &str) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (watch_tx, watch_rx) = mpsc::channel::<Option<Watch>>();
        let (answer_tx, answer_rx) = mpsc::channel::<ForumAnswer>();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        let shared = Arc::new(Shared {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            agent,
            session: Mutex::new(None),
            answers: answer_tx,
        });
        let requests = shared.clone();
        std::thread::spawn(move || {
            // Ends when the client is dropped and the job channel closes.
            while let Ok(job) = job_rx.recv() {
                let result = serve(&requests, &job);
                if requests.answers.send(ForumAnswer::Reply(job.request, result)).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || watch(&shared, &watch_rx));
        Self { jobs: job_tx, watches: watch_tx, answers: answer_rx, watching: None }
    }

    pub(crate) fn request(&self, identity: Identity, request: ForumRequest) -> Result<(), String> {
        self.jobs
            .send(Job { identity, request })
            .map_err(|_| "the forum worker stopped".to_string())
    }

    /// Watches the replies to another character, or to none. Handing over the
    /// character already watched changes nothing.
    pub(crate) fn watch(&mut self, watch: Option<Watch>) {
        let identity = watch.as_ref().map(|w| w.identity.clone());
        if identity != self.watching {
            self.watching = identity;
            let _ = self.watches.send(watch);
        }
    }

    pub(crate) fn watching(&self) -> Option<&Identity> {
        self.watching.as_ref()
    }

    pub(crate) fn poll(&self) -> Vec<ForumAnswer> {
        std::iter::from_fn(|| self.answers.try_recv().ok()).collect()
    }
}

/// Runs one request, signing in first when there is no token for the
/// character, and once more when the server refuses the token it had.
fn serve(shared: &Shared, job: &Job) -> Result<Value, String> {
    let (token, fresh) = shared.token(&job.identity).map_err(Failure::message)?;
    match call(&shared.agent, &shared.base_url, &job.request, &token) {
        Err(Failure::Unauthorized(_)) if !fresh => {
            shared.forget(&token);
            let (token, _) = shared.token(&job.identity).map_err(Failure::message)?;
            call(&shared.agent, &shared.base_url, &job.request, &token).map_err(Failure::message)
        }
        result => result.map_err(Failure::message),
    }
}

/// How a connection of the watcher ended.
enum End {
    /// The client asked to watch another character, or none.
    Retarget(Option<Watch>),
    /// The client is gone.
    Stopped,
    Failed(Failure),
}

/// The watcher thread: keeps the WebSocket of the watched character open,
/// connecting again with growing waits when it drops.
fn watch(shared: &Shared, watches: &Receiver<Option<Watch>>) {
    let mut target: Option<Watch> = None;
    let mut delay = RETRY_MIN;
    loop {
        let Some(current) = target.as_mut() else {
            match watches.recv() {
                Ok(next) => target = next,
                Err(_) => return,
            }
            continue;
        };
        let started = Instant::now();
        match watch_once(shared, current, watches) {
            End::Retarget(next) => {
                target = next;
                delay = RETRY_MIN;
            }
            End::Stopped => return,
            End::Failed(failure) => {
                if started.elapsed() >= STABLE {
                    delay = RETRY_MIN;
                }
                tracing::debug!("forum replies: {}, trying again in {delay:?}", failure.message());
                match watches.recv_timeout(delay) {
                    Ok(next) => {
                        target = next;
                        delay = RETRY_MIN;
                    }
                    Err(RecvTimeoutError::Timeout) => delay = (delay * 2).min(RETRY_MAX),
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// One connection: signs in when needed, opens the socket after the last reply
/// handled, and passes on replies until it drops or the target changes.
fn watch_once(shared: &Shared, target: &mut Watch, watches: &Receiver<Option<Watch>>) -> End {
    let token = match shared.token(&target.identity) {
        Ok((token, _)) => token,
        Err(failure) => return End::Failed(failure),
    };
    let mut socket = match open_socket(&shared.base_url, &token, target.after) {
        Ok(socket) => socket,
        Err(failure) => {
            if let Failure::Unauthorized(_) = failure {
                shared.forget(&token);
            }
            return End::Failed(failure);
        }
    };
    loop {
        match watches.try_recv() {
            Ok(next) => {
                let _ = socket.close(None);
                return End::Retarget(next);
            }
            Err(TryRecvError::Disconnected) => {
                let _ = socket.close(None);
                return End::Stopped;
            }
            Err(TryRecvError::Empty) => {}
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(event) = serde_json::from_str::<Value>(text.as_str()) else {
                    continue;
                };
                if event.get("event").and_then(Value::as_str) != Some("reply") {
                    continue;
                }
                let Some(seq) = event.get("seq").and_then(Value::as_u64) else {
                    continue;
                };
                // A reconnect starts after the largest one handled.
                if seq <= target.after {
                    continue;
                }
                target.after = seq;
                if shared.answers.send(ForumAnswer::Notice(event)).is_err() {
                    return End::Stopped;
                }
            }
            Ok(Message::Close(_)) => return End::Failed(Failure::Other("the server closed the socket".into())),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return End::Failed(Failure::Other(e.to_string())),
        }
    }
}

/// `ws://` or `wss://` for the forum's socket, from its `http(s)://` base.
fn socket_url(base_url: &str, after: u64) -> String {
    let url = join_url(base_url, &format!("/forum/ws?after={after}"));
    match url.split_once("://") {
        Some(("https", rest)) => format!("wss://{rest}"),
        Some((_, rest)) => format!("ws://{rest}"),
        None => format!("ws://{url}"),
    }
}

fn open_socket(base_url: &str, token: &str, after: u64) -> Result<Socket, Failure> {
    let other = |e: &dyn std::fmt::Display| Failure::Other(e.to_string());
    let url = socket_url(base_url, after);
    let mut request = url.as_str().into_client_request().map_err(|e| other(&e))?;
    let bearer = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| other(&e))?;
    request.headers_mut().insert("Authorization", bearer);
    let host = request.uri().host().unwrap_or("127.0.0.1").trim_matches(['[', ']']).to_string();
    let secure = request.uri().scheme_str() == Some("wss");
    let port = request.uri().port_u16().unwrap_or(if secure { 443 } else { 80 });
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| other(&e))?
        .next()
        .ok_or_else(|| Failure::Other(format!("{host} has no address")))?;
    let stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|e| other(&e))?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).map_err(|e| other(&e))?;
    let (socket, _) = tungstenite::client_tls(request, stream).map_err(|e| match e {
        HandshakeError::Failure(tungstenite::Error::Http(response)) if response.status().as_u16() == 401 => {
            Failure::Unauthorized("the forum refused the token".to_string())
        }
        HandshakeError::Failure(e) => other(&e),
        HandshakeError::Interrupted(_) => Failure::Other("the forum did not answer".to_string()),
    })?;
    // Short reads from now on, so the watcher looks at its target often. Set
    // on the stream the socket reads, not a clone: on Windows a cloned handle
    // does not share the timeout.
    let tcp = match socket.get_ref() {
        MaybeTlsStream::Plain(tcp) => tcp,
        MaybeTlsStream::Rustls(tls) => &tls.sock,
        _ => return Err(Failure::Other("unexpected socket kind".to_string())),
    };
    tcp.set_read_timeout(Some(WATCH_TICK)).map_err(|e| other(&e))?;
    Ok(socket)
}

/// Signs in as the character, and answers the token and the character's name.
fn sign_in(agent: &ureq::Agent, base_url: &str, identity: &Identity) -> Result<(String, String), Failure> {
    let body = |account: &str| {
        json!({ "account": account, "password": identity.password, "character": identity.character })
    };
    let issued = match send(agent, base_url, "POST", "/forum/auth", None, Some(body(&identity.account))) {
        // A login name ending in `_M`/`_F` created the account at the game
        // login; the forum knows the account by the name without it.
        Err(Failure::Other(message)) => match registration_name(&identity.account) {
            Some(account) => send(agent, base_url, "POST", "/forum/auth", None, Some(body(account)))?,
            None => return Err(Failure::Other(message)),
        },
        result => result?,
    };
    let token = issued.get("token").and_then(Value::as_str);
    let name = issued.pointer("/character/name").and_then(Value::as_str);
    match (token, name) {
        (Some(token), name) => Ok((token.to_string(), name.unwrap_or(&identity.character).to_string())),
        (None, _) => Err(Failure::Other("the sign-in answer has no token".to_string())),
    }
}

/// `account` without the `_M`/`_F` that asks the game login for a new account.
fn registration_name(account: &str) -> Option<&str> {
    let stem = account.strip_suffix(['M', 'm', 'F', 'f'])?.strip_suffix('_')?;
    (!stem.is_empty()).then_some(stem)
}

fn call(agent: &ureq::Agent, base_url: &str, request: &ForumRequest, token: &str) -> Result<Value, Failure> {
    let (method, path, body) = route(request);
    send(agent, base_url, method, &path, Some(token), body)
}

/// The method, path and body of a request.
fn route(request: &ForumRequest) -> (&'static str, String, Option<Value>) {
    match request {
        ForumRequest::Threads { before, limit } => {
            let mut path = format!("/forum/threads?limit={limit}");
            if let Some(before) = before {
                path.push_str(&format!("&before={before}"));
            }
            ("GET", path, None)
        }
        ForumRequest::Thread { id } => ("GET", format!("/forum/threads/{id}"), None),
        ForumRequest::CreateThread { title, body } => {
            ("POST", "/forum/threads".to_string(), Some(json!({ "title": title, "body": body })))
        }
        ForumRequest::Comment { thread_id, body, parent } => {
            let mut payload = json!({ "body": body });
            if let Some(parent) = parent {
                payload["parent"] = json!(parent);
            }
            ("POST", format!("/forum/threads/{thread_id}/comments"), Some(payload))
        }
    }
}

fn send(
    agent: &ureq::Agent,
    base_url: &str,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Result<Value, Failure> {
    let url = join_url(base_url, path);
    let bearer = token.map(|token| format!("Bearer {token}"));
    let result = if method == "POST" {
        let mut request = agent
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(bearer) = &bearer {
            request = request.header("Authorization", bearer);
        }
        request.send(body.unwrap_or(Value::Null).to_string())
    } else {
        let mut request = agent.get(&url).header("Accept", "application/json");
        if let Some(bearer) = &bearer {
            request = request.header("Authorization", bearer);
        }
        request.call()
    };
    let mut response = result.map_err(|e| Failure::Other(format!("{url}: {e}")))?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| Failure::Other(format!("{url}: {e}")))?;
    match parse_answer(status, &text) {
        Err(message) if status == 401 => Err(Failure::Unauthorized(message)),
        result => result.map_err(Failure::Other),
    }
}

impl App {
    fn forum_base(&self) -> &str {
        self.config.forum_url.as_deref().map(str::trim).unwrap_or("")
    }

    /// The game login and the character in play, when there is one.
    fn forum_identity(&self) -> Option<Identity> {
        let character = self.game.session.selected_character.as_ref()?.name.clone();
        let (account, password) = self.login_credentials.clone()?;
        Some(Identity { account, password, character })
    }

    /// Forwards a forum window request to the worker, started on first use. It
    /// signs in with the game login, so it needs a character in play.
    pub(crate) fn request_forum(&mut self, request: ForumRequest) {
        let base = self.forum_base().to_string();
        if base.is_empty() {
            let message = "no forum_url is set in the config".to_string();
            self.windows.forum_window.deliver(&request, Err(message));
            return;
        }
        let Some(identity) = self.forum_identity() else {
            let message = "sign in to the game first".to_string();
            self.windows.forum_window.deliver(&request, Err(message));
            return;
        };
        let client = self.forum.get_or_insert_with(|| ForumClient::spawn(&base));
        if let Err(message) = client.request(identity, request.clone()) {
            self.windows.forum_window.deliver(&request, Err(message));
        }
    }

    /// Watches the replies to the character in game, then moves the answers
    /// that arrived since the last frame into the window and the chat.
    pub(crate) fn poll_forum(&mut self) {
        self.sync_forum_watch();
        let Some(client) = &self.forum else {
            return;
        };
        let mut notices = Vec::new();
        for answer in client.poll() {
            match answer {
                ForumAnswer::SignedIn(name) => self.windows.forum_window.signed_in(&name),
                ForumAnswer::Reply(request, result) => self.windows.forum_window.deliver(&request, result),
                ForumAnswer::Notice(notice) => notices.push(notice),
            }
        }
        self.show_forum_notices(&notices);
    }

    /// The WebSocket is open while a character is in game, and only then.
    fn sync_forum_watch(&mut self) {
        let base = self.forum_base().to_string();
        let wanted = if self.game.session.app_state == AppState::InGame && !base.is_empty() {
            self.forum_identity()
        } else {
            None
        };
        if wanted.as_ref() == self.forum.as_ref().and_then(ForumClient::watching) {
            return;
        }
        let watch = wanted.map(|identity| {
            let after = self.config.forum_reply_seq.get(&identity.character).copied().unwrap_or(0);
            Watch { identity, after }
        });
        let client = self.forum.get_or_insert_with(|| ForumClient::spawn(&base));
        client.watch(watch);
    }

    /// Keeps the last reply handled for the next session, and tells the player.
    fn show_forum_notices(&mut self, notices: &[Value]) {
        if notices.is_empty() {
            return;
        }
        let character = self.forum.as_ref().and_then(ForumClient::watching).map(|i| i.character.clone());
        let newest = notices.iter().filter_map(|n| n.get("seq").and_then(Value::as_u64)).max();
        if let (Some(character), Some(newest)) = (character, newest) {
            let seen = self.config.forum_reply_seq.entry(character).or_insert(0);
            if newest > *seen {
                *seen = newest;
                self.config.save("config.json");
            }
        }
        for line in self.windows.forum_window.add_notices(notices) {
            self.windows.chat_window.add_system(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn requests_map_to_the_forum_routes() {
        assert_eq!(route(&ForumRequest::Threads { before: None, limit: 25 }).1, "/forum/threads?limit=25");
        assert_eq!(route(&ForumRequest::Threads { before: Some(9), limit: 25 }).1, "/forum/threads?limit=25&before=9");
        assert_eq!(route(&ForumRequest::Thread { id: 4 }), ("GET", "/forum/threads/4".to_string(), None));
        let (method, path, body) =
            route(&ForumRequest::Comment { thread_id: 4, body: "Hi".to_string(), parent: Some(7) });
        assert_eq!((method, path.as_str()), ("POST", "/forum/threads/4/comments"));
        assert_eq!(body.unwrap(), json!({ "body": "Hi", "parent": 7 }));
        let (_, _, body) = route(&ForumRequest::Comment { thread_id: 4, body: "Hi".to_string(), parent: None });
        assert_eq!(body.unwrap(), json!({ "body": "Hi" }));
    }

    #[test]
    fn a_registration_suffix_is_dropped_for_the_forum() {
        assert_eq!(registration_name("bob_M"), Some("bob"));
        assert_eq!(registration_name("bob_f"), Some("bob"));
        assert_eq!(registration_name("bob"), None);
        assert_eq!(registration_name("_M"), None);
        assert_eq!(registration_name("bobM"), None);
    }

    /// Reads one whole request, its body included: answering before the body
    /// is in makes the client's write fail and its next request never come.
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).unwrap();
            data.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&data).into_owned();
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text[..end]
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().ok())?
                    })
                    .unwrap_or(0usize);
                if data.len() >= end + 4 + length || n == 0 {
                    return text;
                }
            } else if n == 0 {
                return text;
            }
        }
    }

    /// A server answering each connection with the next canned reply, which
    /// hands back the requests it read.
    fn serve_replies(replies: Vec<(u16, &'static str)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                seen.push(read_request(&mut stream));
                let reply = format!(
                    "HTTP/1.1 {status} X{crlf}Content-Type: application/json{crlf}Content-Length: {}{crlf}Connection: close{crlf}{crlf}{body}",
                    body.len(),
                    crlf = "\r\n",
                );
                stream.write_all(reply.as_bytes()).unwrap();
            }
            seen
        });
        (base, handle)
    }

    fn wait_for(client: &ForumClient, count: usize) -> Vec<ForumAnswer> {
        let mut answers = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while answers.len() < count {
            assert!(std::time::Instant::now() < deadline, "only {} of {count} answers came", answers.len());
            answers.extend(client.poll());
            std::thread::sleep(Duration::from_millis(20));
        }
        answers
    }

    fn hero() -> Identity {
        Identity { account: "alice".into(), password: "secret".into(), character: "hero".into() }
    }

    #[test]
    fn the_worker_signs_in_then_sends_the_token() {
        let (base, server) = serve_replies(vec![
            (200, r#"{"token":"t1","character":{"char_id":1,"name":"Hero"},"expires_at":0}"#),
            (200, r#"[{"id":1,"title":"Hi"}]"#),
        ]);
        let client = ForumClient::spawn(&base);
        client.request(hero(), ForumRequest::Threads { before: None, limit: 25 }).unwrap();
        let answers = wait_for(&client, 2);
        assert!(matches!(&answers[0], ForumAnswer::SignedIn(name) if name == "Hero"));
        let ForumAnswer::Reply(_, Ok(value)) = &answers[1] else { panic!("no reply") };
        assert_eq!(value[0]["title"], "Hi");
        let seen = server.join().unwrap();
        assert!(seen[0].starts_with("POST /forum/auth "));
        assert!(seen[0].contains(r#""account":"alice""#) && seen[0].contains(r#""character":"hero""#));
        assert!(seen[1].starts_with("GET /forum/threads?limit=25 "));
        assert!(seen[1].contains("Bearer t1"));
    }

    #[test]
    fn a_refused_token_signs_in_again_once() {
        let (base, server) = serve_replies(vec![
            (200, r#"{"token":"t1","character":{"char_id":1,"name":"Hero"},"expires_at":0}"#),
            (200, r#"{"id":3,"title":"Hi","comments":[]}"#),
            (401, r#"{"error":"Sign in"}"#),
            (200, r#"{"token":"t2","character":{"char_id":1,"name":"Hero"},"expires_at":0}"#),
            (200, r#"{"id":3,"title":"Hi","comments":[]}"#),
        ]);
        let client = ForumClient::spawn(&base);
        client.request(hero(), ForumRequest::Thread { id: 3 }).unwrap();
        wait_for(&client, 2);
        client.request(hero(), ForumRequest::Thread { id: 3 }).unwrap();
        let answers = wait_for(&client, 2);
        assert!(matches!(&answers[1], ForumAnswer::Reply(_, Ok(_))));
        let seen = server.join().unwrap();
        assert!(seen[2].contains("Bearer t1"));
        assert!(seen[3].starts_with("POST /forum/auth "));
        assert!(seen[4].contains("Bearer t2"));
    }

    #[test]
    fn a_failed_sign_in_reports_the_server_message() {
        let (base, server) = serve_replies(vec![(401, r#"{"error":"Wrong password"}"#)]);
        let client = ForumClient::spawn(&base);
        client.request(hero(), ForumRequest::Threads { before: None, limit: 25 }).unwrap();
        let answers = wait_for(&client, 1);
        let ForumAnswer::Reply(_, Err(message)) = &answers[0] else { panic!("no error") };
        assert_eq!(message, "Wrong password");
        server.join().unwrap();
    }

    #[test]
    fn the_socket_url_follows_the_scheme_of_the_base() {
        assert_eq!(socket_url("http://127.0.0.1:6902", 0), "ws://127.0.0.1:6902/forum/ws?after=0");
        assert_eq!(socket_url("https://ro.example/", 812), "wss://ro.example/forum/ws?after=812");
    }

    #[test]
    fn the_watcher_signs_in_opens_the_socket_after_the_last_reply_and_passes_new_ones_on() {
        use tungstenite::handshake::server::{Request, Response};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let auth = read_request(&mut stream);
            let body = r#"{"token":"t1","character":{"char_id":1,"name":"Hero"},"expires_at":0}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            drop(stream);

            let (stream, _) = listener.accept().unwrap();
            // Fails the test rather than hanging it if the watcher never lets go.
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            let mut upgrade = (String::new(), String::new());
            let mut socket = tungstenite::accept_hdr(stream, |request: &Request, response: Response| {
                upgrade.0 = request.uri().to_string();
                upgrade.1 = request
                    .headers()
                    .get("Authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                Ok(response)
            })
            .unwrap();
            for (seq, body) in [(3, "Seen before"), (5, "Does the offer still stand?")] {
                let event = json!({ "event": "reply", "for": "Hero", "seq": seq, "kind": "thread_reply",
                    "thread_id": 40, "comment_id": seq, "parent": null, "from": "Bob", "body": body,
                    "created_at": 1_760_000_000 });
                socket.send(Message::text(event.to_string())).unwrap();
            }
            // Held open until the watcher lets go of it.
            let closed = loop {
                match socket.read() {
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e))
                        if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        break false;
                    }
                    Err(_) => break true,
                }
            };
            (auth, upgrade, closed)
        });

        let mut client = ForumClient::spawn(&base);
        client.watch(Some(Watch { identity: hero(), after: 3 }));
        let answers = wait_for(&client, 2);
        assert!(matches!(&answers[0], ForumAnswer::SignedIn(name) if name == "Hero"));
        let ForumAnswer::Notice(notice) = &answers[1] else { panic!("no notice") };
        assert_eq!(notice["seq"], 5, "a reply at or before `after` is not passed on again");
        assert_eq!(notice["from"], "Bob");

        client.watch(None);
        let (auth, (uri, bearer), closed) = server.join().unwrap();
        assert!(closed, "watching no one closes the socket");
        assert!(auth.starts_with("POST /forum/auth "));
        assert_eq!(uri, "/forum/ws?after=3");
        assert_eq!(bearer, "Bearer t1");
        assert!(client.poll().is_empty());
    }
}
