use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::FormatError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Covers the largest download, a BGM track, on a slow link.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Cap on one response body, compressed or not.
const MAX_BODY_BYTES: u64 = 512 * 1024 * 1024;
/// Per-file checksum list, one JSON object per line.
const CHECK: &str = "check";
/// Older servers only publish this, with the same entries under `files`.
const MANIFEST: &str = "manifest.json";
/// The last list received, so a cached client still starts when the server is down.
const SAVED_LIST: &str = "list.jsonl";
const OBJECTS: &str = "objects";
/// Leftover temporary files older than this are removed by the startup sweep.
const STALE_TEMP: Duration = Duration::from_secs(24 * 60 * 60);

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One served file: `size` and `md5` describe the decompressed content.
#[derive(Serialize, Deserialize)]
struct ListEntry {
    path: String,
    size: u64,
    md5: String,
}

#[derive(Deserialize)]
struct Manifest {
    files: Vec<ListEntry>,
}

struct Entry {
    size: u64,
    /// Lowercase hex digest naming the cached object; empty when the server sent
    /// something that is not an md5, which keeps the file out of the cache.
    md5: String,
}

/// Client data served over HTTP by the server's `/data` endpoint.
///
/// The endpoint mirrors the inside of a GRF's `data/` folder, lowercased
/// (`sprite/…`, `texture/…`, `prontera.rsw`, `bgm/…`). `/data/check` lists every
/// file with its size and md5, which we fetch once at connect: asking whether a
/// file exists never touches the network, and a missing file fails without a
/// round trip.
///
/// Compressible files are stored on the server only as `<name>.zst` and sent
/// with `Content-Encoding: zstd`, which we request on every call and decode
/// here. Already-compressed formats (mp3, png, jpeg) come back as stored.
///
/// With a cache directory, every file is also kept there, named by its md5, so
/// it is downloaded once. The list received at connect decides what is current:
/// a file the server changed has a new md5 and is fetched again, and objects no
/// list entry points to are deleted. Without a cache every read is a download.
pub struct RemoteData {
    base_url: String,
    agent: ureq::Agent,
    files: HashMap<String, Entry>,
    cache: Option<PathBuf>,
    /// The list came from the cache because the server was unreachable, so only
    /// cached files can be read.
    offline: bool,
}

impl RemoteData {
    /// Fetches the file list of the endpoint at `base_url`, for instance
    /// `http://127.0.0.1:6903/data`. `cache_dir` is the root of the on-disk
    /// cache; this endpoint's files live in a subfolder of it.
    pub fn connect(base_url: &str, cache_dir: Option<&Path>) -> Result<Self, FormatError> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        let base_url = base_url.trim_end_matches('/').to_string();
        let cache = cache_dir.and_then(|root| {
            let dir = root.join(server_folder(&base_url));
            match std::fs::create_dir_all(dir.join(OBJECTS)) {
                Ok(()) => Some(dir),
                Err(e) => {
                    tracing::warn!("Client data cache disabled, {}: {e}", dir.display());
                    None
                }
            }
        });
        let mut remote = RemoteData {
            base_url,
            agent,
            files: HashMap::new(),
            cache,
            offline: false,
        };

        match remote.fetch_list() {
            Ok(entries) => {
                remote.files = index(entries);
                remote.save_list();
                remote.prune();
            }
            Err(e) => {
                let saved = remote.saved_list();
                let Some(entries) = saved else { return Err(e) };
                tracing::warn!(
                    "Client data server unreachable, using the cached copy: {}",
                    match &e {
                        FormatError::Io(e) => e.to_string(),
                        other => format!("{other:?}"),
                    }
                );
                remote.files = index(entries);
                remote.offline = true;
            }
        }
        Ok(remote)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// True when the server could not be reached and only cached files are served.
    pub fn is_offline(&self) -> bool {
        self.offline
    }

    /// The folder holding this endpoint's cache, when caching is on.
    pub fn cache_dir(&self) -> Option<&Path> {
        self.cache.as_deref()
    }

    /// `key` is a normalized path: lowercase, `/` separators, no leading `data/`.
    /// Offline, only files that are in the cache count.
    pub fn contains(&self, key: &str) -> bool {
        match self.files.get(key) {
            Some(entry) if self.offline => self.cached_path(entry).is_some_and(|p| p.is_file()),
            Some(_) => true,
            None => false,
        }
    }

    /// Returns `key` decompressed, from the cache when it holds the current
    /// version and from the server otherwise. Unlisted files are `NotFound`
    /// without any request.
    pub fn read(&self, key: &str) -> Result<Vec<u8>, FormatError> {
        let Some(entry) = self.files.get(key) else {
            return Err(FormatError::Io(Error::new(
                ErrorKind::NotFound,
                format!("file not served by {}: {key}", self.base_url),
            )));
        };

        let cached = self.cached_path(entry);
        if let Some(path) = &cached {
            match std::fs::read(path) {
                Ok(data) if data.len() as u64 == entry.size => return Ok(data),
                Ok(_) => {
                    // Truncated or damaged: drop it and download again.
                    let _ = std::fs::remove_file(path);
                }
                Err(_) => {}
            }
        }
        if self.offline {
            return Err(FormatError::Io(Error::new(
                ErrorKind::NotFound,
                format!("{key} is not cached and {} is unreachable", self.base_url),
            )));
        }

        let data = self.fetch(key)?;
        if let Some(path) = cached {
            // A different length means the server changed the file after the
            // list was fetched: use it this time, but do not file it under the
            // old md5.
            if data.len() as u64 == entry.size {
                store(&path, &data);
            }
        }
        Ok(data)
    }

    fn cached_path(&self, entry: &Entry) -> Option<PathBuf> {
        let cache = self.cache.as_ref()?;
        (!entry.md5.is_empty()).then(|| cache.join(OBJECTS).join(&entry.md5[..2]).join(&entry.md5))
    }

    /// `/data/check`, or the manifest when the server predates it.
    fn fetch_list(&self) -> Result<Vec<ListEntry>, FormatError> {
        match self.fetch(CHECK) {
            Ok(bytes) => parse_jsonl(&bytes, &self.base_url),
            Err(FormatError::Io(e)) if e.kind() == ErrorKind::NotFound => {
                let bytes = self.fetch(MANIFEST)?;
                let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| {
                    FormatError::Io(Error::new(
                        ErrorKind::InvalidData,
                        format!("{MANIFEST} of {} is not valid: {e}", self.base_url),
                    ))
                })?;
                Ok(manifest.files)
            }
            Err(e) => Err(e),
        }
    }

    fn saved_list(&self) -> Option<Vec<ListEntry>> {
        let bytes = std::fs::read(self.cache.as_ref()?.join(SAVED_LIST)).ok()?;
        parse_jsonl(&bytes, &self.base_url).ok().filter(|entries| !entries.is_empty())
    }

    fn save_list(&self) {
        let Some(cache) = &self.cache else { return };
        let mut jsonl = Vec::new();
        for (path, entry) in &self.files {
            let line = ListEntry {
                path: path.clone(),
                size: entry.size,
                md5: entry.md5.clone(),
            };
            if serde_json::to_writer(&mut jsonl, &line).is_ok() {
                jsonl.push(b'\n');
            }
        }
        store(&cache.join(SAVED_LIST), &jsonl);
    }

    /// Deletes cached objects the current list no longer points to, which are
    /// the previous versions of files the server changed or removed.
    fn prune(&self) {
        let Some(cache) = &self.cache else { return };
        let wanted: HashSet<&str> = self.files.values().map(|e| e.md5.as_str()).collect();
        let mut removed = 0usize;
        let Ok(buckets) = std::fs::read_dir(cache.join(OBJECTS)) else { return };
        for bucket in buckets.flatten() {
            let Ok(objects) = std::fs::read_dir(bucket.path()) else { continue };
            for object in objects.flatten() {
                let name = object.file_name();
                let name = name.to_string_lossy();
                let stale = if name.starts_with(".tmp") {
                    // Another client may be writing one right now.
                    object
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > STALE_TEMP)
                } else {
                    !wanted.contains(name.as_ref())
                };
                if stale && std::fs::remove_file(object.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            tracing::info!("Removed {removed} out-of-date file(s) from the client data cache");
        }
    }

    fn fetch(&self, key: &str) -> Result<Vec<u8>, FormatError> {
        let url = format!("{}/{}", self.base_url, encode_path(key));
        let mut response = self
            .agent
            .get(&url)
            .header("Accept-Encoding", "zstd")
            .call()
            .map_err(|e| FormatError::Io(Error::other(format!("GET {url}: {e}"))))?;

        let status = response.status();
        if !status.is_success() {
            let kind = if status.as_u16() == 404 {
                ErrorKind::NotFound
            } else {
                ErrorKind::Other
            };
            return Err(FormatError::Io(Error::new(
                kind,
                format!("GET {url}: HTTP {status}"),
            )));
        }

        let zstd_encoded = response
            .headers()
            .get("content-encoding")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("zstd"));
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .map_err(|e| FormatError::Io(Error::other(format!("GET {url}: {e}"))))?;

        if zstd_encoded {
            zstd::stream::decode_all(body.as_slice())
                .map_err(|e| FormatError::DecompressionFailed(format!("{url}: {e}")))
        } else {
            Ok(body)
        }
    }
}

fn parse_jsonl(bytes: &[u8], base_url: &str) -> Result<Vec<ListEntry>, FormatError> {
    let invalid = |detail: String| {
        FormatError::Io(Error::new(
            ErrorKind::InvalidData,
            format!("{CHECK} list of {base_url} is not valid: {detail}"),
        ))
    };
    let text = std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|e| invalid(e.to_string())))
        .collect()
}

fn index(entries: Vec<ListEntry>) -> HashMap<String, Entry> {
    entries
        .into_iter()
        .map(|file| {
            let md5 = file.md5.to_ascii_lowercase();
            let valid = md5.len() == 32 && md5.bytes().all(|b| b.is_ascii_hexdigit());
            (
                file.path.to_lowercase().replace('\\', "/"),
                Entry {
                    size: file.size,
                    md5: if valid { md5 } else { String::new() },
                },
            )
        })
        .collect()
}

/// Writes through a temporary file and a rename, so a reader never sees half a
/// file. Failing to cache is not an error: the data was already delivered.
fn store(path: &Path, data: &[u8]) {
    let Some(dir) = path.parent() else { return };
    let temp = dir.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&temp, data))
        .and_then(|()| std::fs::rename(&temp, path));
    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp);
        tracing::debug!("Not cached, {}: {e}", path.display());
    }
}

/// Folder name for one endpoint's cache: `http://host:6903/data` becomes
/// `host_6903_data`.
fn server_folder(base_url: &str) -> String {
    let rest = base_url.split_once("://").map_or(base_url, |(_, rest)| rest);
    let name: String = rest
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    name.trim_matches('_').to_string()
}

/// Where the cache lives unless configured otherwise: `%APPDATA%\nostalro\data`
/// on Windows, `$XDG_CACHE_HOME` or `~/.cache` elsewhere.
pub fn default_cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("nostalro").join("data"))
}

/// True when the URL points at this machine (`localhost`, `127.x`, `::1`), where
/// the files are as close as a cache would put them.
pub fn is_loopback_url(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => authority.split(':').next().unwrap_or(""),
    };
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Percent-encodes each UTF-8 byte of `path` outside the unreserved set,
/// keeping `/`. Client paths carry Korean folder names, spaces and `@`.
fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const MD5_A: &str = "0123456789abcdef0123456789abcdef";
    const MD5_B: &str = "fedcba9876543210fedcba9876543210";

    type Route = (&'static str, Vec<u8>, Option<&'static str>);

    #[test]
    fn encodes_spaces_at_signs_and_korean() {
        assert_eq!(encode_path("sprite/poring.spr"), "sprite/poring.spr");
        assert_eq!(encode_path("lua files/a.lub"), "lua%20files/a.lub");
        assert_eq!(encode_path("1@cash.gat"), "1%40cash.gat");
        assert_eq!(encode_path("유저/a.bmp"), "%EC%9C%A0%EC%A0%80/a.bmp");
    }

    #[test]
    fn recognises_loopback_urls() {
        for url in [
            "http://127.0.0.1:6903/data",
            "http://localhost/data",
            "http://LOCALHOST:6903/data",
            "http://[::1]:6903/data",
            "http://127.1.2.3/data",
            "http://user@localhost:1/data",
        ] {
            assert!(is_loopback_url(url), "{url}");
        }
        for url in [
            "http://192.168.1.10:6903/data",
            "https://play.example.com/data",
            "http://localhost.example.com/data",
            "http://10.0.0.1/data",
        ] {
            assert!(!is_loopback_url(url), "{url}");
        }
    }

    #[test]
    fn names_the_cache_folder_after_the_server() {
        assert_eq!(server_folder("http://play.example.com:6903/data"), "play.example.com_6903_data");
        assert_eq!(server_folder("http://[::1]:80/data"), "1__80_data");
    }

    /// Serves `routes` over plain HTTP until the process ends, answering one
    /// request per connection, and records every requested path.
    fn serve(routes: Vec<Route>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
                log.lock().unwrap().push(path.clone());
                let accepts_zstd = request.to_lowercase().contains("accept-encoding: zstd");
                let route = routes.iter().find(|(p, _, _)| *p == path);
                let (status, body, encoding) = match route {
                    Some((_, body, Some(encoding))) if accepts_zstd => {
                        ("200 OK", body.clone(), Some(*encoding))
                    }
                    Some((_, _, Some(_))) => ("404 Not Found", Vec::new(), None),
                    Some((_, body, None)) => ("200 OK", body.clone(), None),
                    None => ("404 Not Found", Vec::new(), None),
                };
                let mut head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len()
                );
                if let Some(encoding) = encoding {
                    head.push_str(&format!("Content-Encoding: {encoding}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        (format!("http://{addr}/data"), requests)
    }

    fn jsonl(entries: &[(&str, u64, &str)]) -> Vec<u8> {
        entries
            .iter()
            .map(|(path, size, md5)| {
                format!("{{\"path\":\"{path}\",\"size\":{size},\"md5\":\"{md5}\"}}\n")
            })
            .collect::<String>()
            .into_bytes()
    }

    fn temp_cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nostalro_remote_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn file_requests(requests: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| !p.ends_with("/check") && !p.ends_with("manifest.json"))
            .cloned()
            .collect()
    }

    fn not_found(result: Result<Vec<u8>, FormatError>) {
        match result {
            Err(FormatError::Io(e)) => assert_eq!(e.kind(), ErrorKind::NotFound),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn reads_stored_and_zstd_files_and_skips_unlisted_ones() {
        let list = jsonl(&[
            ("sprite/a.spr", 3, MD5_A),
            ("bgm/01.mp3", 3, MD5_B),
            ("missing.txt", 1, MD5_A),
        ]);
        let (url, requests) = serve(vec![
            ("/data/check", zstd::bulk::compress(&list, 3).unwrap(), Some("zstd")),
            ("/data/sprite/a.spr", zstd::bulk::compress(b"SPR", 3).unwrap(), Some("zstd")),
            ("/data/bgm/01.mp3", b"MP3".to_vec(), None),
        ]);

        let remote = RemoteData::connect(&url, None).unwrap();
        assert_eq!(remote.file_count(), 3);
        assert!(remote.contains("sprite/a.spr"));
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        assert_eq!(remote.read("bgm/01.mp3").unwrap(), b"MP3");

        // Listed but not served: the server's 404 surfaces as NotFound.
        not_found(remote.read("missing.txt"));
        // Unlisted: refused without a request.
        let before = requests.lock().unwrap().len();
        not_found(remote.read("sprite/nope.spr"));
        assert_eq!(requests.lock().unwrap().len(), before);
    }

    #[test]
    fn falls_back_to_the_manifest_without_a_check_list() {
        let manifest = format!(
            "{{\"files\":[{{\"path\":\"sprite/a.spr\",\"size\":3,\"md5\":\"{MD5_A}\"}}]}}"
        );
        let (url, _) = serve(vec![
            ("/data/manifest.json", manifest.into_bytes(), None),
            ("/data/sprite/a.spr", b"SPR".to_vec(), None),
        ]);
        let remote = RemoteData::connect(&url, None).unwrap();
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
    }

    #[test]
    fn connect_fails_when_the_server_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(RemoteData::connect(&format!("http://{addr}/data"), None).is_err());
    }

    #[test]
    fn downloads_once_then_serves_from_the_cache() {
        let (url, requests) = serve(vec![
            ("/data/check", jsonl(&[("sprite/a.spr", 3, MD5_A)]), None),
            ("/data/sprite/a.spr", b"SPR".to_vec(), None),
        ]);
        let cache = temp_cache("hit");

        let remote = RemoteData::connect(&url, Some(&cache)).unwrap();
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        assert_eq!(file_requests(&requests).len(), 1);

        // A new session reuses what the last one stored.
        let remote = RemoteData::connect(&url, Some(&cache)).unwrap();
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        assert_eq!(file_requests(&requests).len(), 1);

        // A damaged object is downloaded again.
        let object = remote.cached_path(&remote.files["sprite/a.spr"]).unwrap();
        std::fs::write(&object, b"X").unwrap();
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        assert_eq!(file_requests(&requests).len(), 2);

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_changed_checksum_refetches_and_removes_the_old_version() {
        let cache = temp_cache("stale");
        let (url_old, _) = serve(vec![
            ("/data/check", jsonl(&[("sprite/a.spr", 3, MD5_A)]), None),
            ("/data/sprite/a.spr", b"OLD".to_vec(), None),
        ]);
        // Same cache folder as the old server: reuse the folder name by hand.
        let remote = RemoteData::connect(&url_old, Some(&cache)).unwrap();
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"OLD");
        let folder = remote.cache_dir().unwrap().to_path_buf();
        let old_object = remote.cached_path(&remote.files["sprite/a.spr"]).unwrap();
        assert!(old_object.is_file());

        let (url_new, requests) = serve(vec![
            ("/data/check", jsonl(&[("sprite/a.spr", 3, MD5_B)]), None),
            ("/data/sprite/a.spr", b"NEW".to_vec(), None),
        ]);
        // Point the new server at the old one's folder.
        let moved = cache.join(server_folder(&url_new));
        std::fs::rename(&folder, &moved).unwrap();

        let remote = RemoteData::connect(&url_new, Some(&cache)).unwrap();
        assert!(!moved.join(OBJECTS).join(&MD5_A[..2]).join(MD5_A).exists(), "old version kept");
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"NEW");
        assert_eq!(file_requests(&requests).len(), 1);

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn serves_cached_files_when_the_server_is_down() {
        let cache = temp_cache("offline");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let url = format!("http://{addr}/data");

        // Seed what an earlier online session would have left behind.
        let folder = cache.join(server_folder(&url));
        std::fs::create_dir_all(folder.join(OBJECTS)).unwrap();
        std::fs::write(
            folder.join(SAVED_LIST),
            jsonl(&[("sprite/a.spr", 3, MD5_A), ("sprite/b.spr", 3, MD5_B)]),
        )
        .unwrap();
        store(&folder.join(OBJECTS).join(&MD5_A[..2]).join(MD5_A), b"SPR");

        let remote = RemoteData::connect(&url, Some(&cache)).unwrap();
        assert!(remote.is_offline());
        assert!(remote.contains("sprite/a.spr"));
        assert!(!remote.contains("sprite/b.spr"), "uncached files cannot be read offline");
        assert_eq!(remote.read("sprite/a.spr").unwrap(), b"SPR");
        not_found(remote.read("sprite/b.spr"));

        let _ = std::fs::remove_dir_all(&cache);
    }
}
