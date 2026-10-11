//! Fetches pages of the server's read-only reference API (`/db`) for the
//! database window. Requests run one at a time on a worker thread so a slow or
//! dead server never stalls a frame; answers are collected by `poll`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde_json::Value;

use crate::App;
use crate::sprite::thumbnail;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Pictures loaded per frame; a remote file is read on the frame thread.
const TEXTURES_PER_FRAME: usize = 2;

type Answer = (String, Result<Value, String>);

pub(crate) struct ReferenceClient {
    requests: Sender<String>,
    answers: Receiver<Answer>,
}

impl ReferenceClient {
    /// `base_url` is the reference server, for instance `http://127.0.0.1:6904`.
    pub(crate) fn spawn(base_url: &str) -> Self {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        let (request_tx, request_rx) = mpsc::channel::<String>();
        let (answer_tx, answer_rx) = mpsc::channel::<Answer>();
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_connect(Some(CONNECT_TIMEOUT))
                .timeout_global(Some(REQUEST_TIMEOUT))
                .http_status_as_error(false)
                .build()
                .into();
            // Ends when the client is dropped and the request channel closes.
            while let Ok(path) = request_rx.recv() {
                let result = fetch(&agent, &base_url, &path);
                if answer_tx.send((path, result)).is_err() {
                    break;
                }
            }
        });
        Self { requests: request_tx, answers: answer_rx }
    }

    pub(crate) fn request(&self, path: String) -> Result<(), String> {
        self.requests.send(path).map_err(|_| "the reference worker stopped".to_string())
    }

    pub(crate) fn poll(&self) -> Vec<Answer> {
        std::iter::from_fn(|| self.answers.try_recv().ok()).collect()
    }
}

fn fetch(agent: &ureq::Agent, base_url: &str, path: &str) -> Result<Value, String> {
    let url = join_url(base_url, path);
    let mut response = agent
        .get(&url)
        .header("Accept", "application/json")
        .call()
        .map_err(|e| format!("{url}: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("{url}: {e}"))?;
    parse_answer(status, &body)
}

fn join_url(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// The server answers errors as `{ "error": "..." }` with a 4xx status.
fn parse_answer(status: u16, body: &str) -> Result<Value, String> {
    let parsed = serde_json::from_str::<Value>(body);
    if let Ok(value) = &parsed
        && let Some(message) = value.get("error").and_then(Value::as_str)
    {
        return Err(message.to_string());
    }
    let ok_status = (200..300).contains(&status);
    match parsed {
        Ok(value) if ok_status => Ok(value),
        Err(e) if ok_status => Err(format!("unreadable answer: {e}")),
        _ => Err(format!("HTTP {status}")),
    }
}

impl App {
    /// Forwards a database window request to the worker, started on first use.
    pub(crate) fn request_reference(&mut self, path: String) {
        let base = self.config.reference_url.as_deref().map(str::trim).unwrap_or("");
        if base.is_empty() {
            self.windows
                .reference_window
                .deliver(&path, Err("no reference_url is set in the config".to_string()));
            return;
        }
        let client = self
            .reference
            .get_or_insert_with(|| ReferenceClient::spawn(base));
        if let Err(message) = client.request(path.clone()) {
            self.windows.reference_window.deliver(&path, Err(message));
        }
    }

    /// Moves the answers that arrived since the last frame into the window and
    /// loads a few of the pictures it asked for.
    pub(crate) fn poll_reference(&mut self) {
        if let Some(client) = &self.reference {
            for (path, result) in client.poll() {
                self.windows.reference_window.deliver(&path, result);
            }
        }
        for _ in 0..TEXTURES_PER_FRAME {
            let Some(path) = self.reference_textures.pop_front() else {
                break;
            };
            let size = self.load_reference_texture(&path);
            self.windows.reference_window.texture_loaded(&path, size);
        }
    }

    /// Makes `path` drawable and returns its size, or `None` when it cannot be
    /// built. `path` is an icon file or a sprite thumbnail key.
    fn load_reference_texture(&mut self, path: &str) -> Option<(u32, u32)> {
        let (grf, renderer) = (self.grf.as_ref()?, self.renderer.as_mut()?);
        if let Some(base) = ragnarok_game::sprite_path::sprite_thumbnail_base(path) {
            if renderer.texture_cache.texture_size(path).is_none() {
                let spr = grf.read_file(&format!("{base}.spr")).ok()?;
                let act = grf.read_file(&format!("{base}.act")).ok()?;
                let thumb = thumbnail::from_files(&spr, &act)?;
                renderer.register_rgba_texture(path, &thumb.rgba, thumb.width, thumb.height);
            }
        } else {
            renderer.preload_textures(&[path], grf);
        }
        renderer.texture_cache.texture_size(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_join_without_doubled_slashes() {
        assert_eq!(join_url("http://h:6904/", "/db/items?q=a"), "http://h:6904/db/items?q=a");
        assert_eq!(join_url("http://h:6904", "db"), "http://h:6904/db");
    }

    #[test]
    fn success_parses_the_body() {
        let value = parse_answer(200, r#"{"total":1}"#).unwrap();
        assert_eq!(value["total"], 1);
    }

    #[test]
    fn server_errors_use_their_message() {
        assert_eq!(parse_answer(404, r#"{"error":"unknown item"}"#).unwrap_err(), "unknown item");
        assert_eq!(parse_answer(200, r#"{"error":"odd"}"#).unwrap_err(), "odd");
    }

    #[test]
    fn bad_bodies_and_statuses_are_reported() {
        assert!(parse_answer(200, "not json").unwrap_err().starts_with("unreadable answer"));
        assert_eq!(parse_answer(502, "<html>").unwrap_err(), "HTTP 502");
    }

    #[test]
    fn worker_fetches_json_from_a_server() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).into_owned();
            let body = r#"{"total":2}"#;
            let reply = format!(
                "HTTP/1.1 200 OK{crlf}Content-Type: application/json{crlf}Content-Length: {}{crlf}Connection: close{crlf}{crlf}{body}",
                body.len(),
                crlf = "\r\n",
            );
            stream.write_all(reply.as_bytes()).unwrap();
            request
        });

        let client = ReferenceClient::spawn(&format!("http://127.0.0.1:{port}/"));
        client.request("/db/items?q=red".to_string()).unwrap();
        let answers = loop {
            let answers = client.poll();
            if !answers.is_empty() {
                break answers;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(answers[0].1.as_ref().unwrap()["total"], 2);
        assert!(server.join().unwrap().starts_with("GET /db/items?q=red HTTP/1.1"));
    }

    #[test]
    fn worker_reports_an_unreachable_server() {
        // Port 1 on loopback refuses the connection at once.
        let client = ReferenceClient::spawn("http://127.0.0.1:1");
        client.request("/db".to_string()).unwrap();
        let answers = loop {
            let answers = client.poll();
            if !answers.is_empty() {
                break answers;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(answers[0].0, "/db");
        assert!(answers[0].1.is_err());
    }
}
