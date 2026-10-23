//! What the client needs from the browser in the web build: the page's
//! storage, its canvas, the console and HTTP.

use std::io::Write;

use wasm_bindgen::JsCast;
use winit::window::WindowAttributes;

/// Local storage key of the saved settings.
pub(crate) const CONFIG_KEY: &str = "nostalro.config";

/// Settings the site's `config.json` decides over the saved ones: where the
/// servers are, which the site may move.
pub(crate) const SITE_KEYS: &[&str] = &[
    "login_servers",
    "data_url",
    "reference_url",
    "forum_url",
    "websocket_url",
];

/// `text` with `{host}` replaced by the page's host name, so a site config
/// can name the server's other ports wherever the page is served from.
pub(crate) fn with_page_host(text: &str) -> String {
    if !text.contains("{host}") {
        return text.to_string();
    }
    let host = web_sys::window().and_then(|w| w.location().hostname().ok());
    text.replace("{host}", host.as_deref().unwrap_or("127.0.0.1"))
}

/// `url` against the page when it is a path (`/data`), so the client can be
/// served from the same origin as the server's endpoints; `{host}` is the
/// page's host name.
pub(crate) fn absolute_url(url: &str) -> String {
    let url = with_page_host(url);
    if !url.starts_with('/') {
        return url;
    }
    let origin = web_sys::window().and_then(|w| w.location().origin().ok());
    match origin {
        Some(origin) => format!("{origin}{url}"),
        None => url.to_string(),
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub(crate) fn storage_get(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok().flatten()
}

pub(crate) fn storage_set(key: &str, value: &str) {
    if let Some(storage) = local_storage()
        && let Err(e) = storage.set_item(key, value)
    {
        tracing::warn!("Could not save {key}: {e:?}");
    }
}

/// `url`'s body when it answers 200, fetched synchronously: the client reads
/// its settings before the event loop starts.
pub(crate) fn get_text_sync(url: &str) -> Option<String> {
    let xhr = web_sys::XmlHttpRequest::new().ok()?;
    xhr.open_with_async("GET", url, false).ok()?;
    xhr.send().ok()?;
    if xhr.status().ok()? != 200 {
        return None;
    }
    xhr.response_text().ok().flatten()
}

/// The page's query parameter `name`.
pub(crate) fn query_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(name)
}

/// One console line per log event, errors and warnings at their own level so
/// the browser's filters work.
struct ConsoleLine {
    level: tracing::Level,
    text: Vec<u8>,
}

impl Write for ConsoleLine {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.text.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for ConsoleLine {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.text);
        let text = wasm_bindgen::JsValue::from_str(text.trim_end());
        match self.level {
            tracing::Level::ERROR => web_sys::console::error_1(&text),
            tracing::Level::WARN => web_sys::console::warn_1(&text),
            _ => web_sys::console::log_1(&text),
        }
    }
}

struct Console;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Console {
    type Writer = ConsoleLine;

    fn make_writer(&'a self) -> Self::Writer {
        ConsoleLine { level: tracing::Level::INFO, text: Vec::new() }
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        ConsoleLine { level: *meta.level(), text: Vec::new() }
    }
}

/// Logs to the browser console, at the level of the page's `?log=` parameter
/// (an `EnvFilter` directive, `info` by default). Panics are logged too.
pub(crate) fn init_logging() {
    console_error_panic_hook::set_once();
    let filter = query_param("log").unwrap_or_else(|| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&filter)
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(Console)
        .with_ansi(false)
        // The default timer reads `SystemTime`, which panics in a browser.
        .without_time()
        .init();
}

/// The game draws into the page's `<canvas id="nostalro">`, or a canvas added
/// to `<body>` when there is none. The page's stylesheet sizes it.
pub(crate) fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    use winit::platform::web::WindowAttributesExtWebSys;
    let canvas = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("nostalro"))
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
    match canvas {
        Some(canvas) => attrs.with_canvas(Some(canvas)),
        None => attrs.with_append(true),
    }
}

pub(crate) async fn sleep(duration: std::time::Duration) {
    let ms = duration.as_millis().min(i32::MAX as u128) as i32;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(window) = web_sys::window() {
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(resolve.unchecked_ref(), ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

enum SocketEvent {
    Open,
    Text(String),
    Closed(String),
}

/// A WebSocket of text messages, closed when dropped.
pub(crate) struct TextSocket {
    socket: web_sys::WebSocket,
    events: tokio::sync::mpsc::UnboundedReceiver<SocketEvent>,
    _handlers: [wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>; 3],
}

impl TextSocket {
    /// Resolves once the socket is open; the browser does not say why one
    /// could not be.
    pub(crate) async fn open(url: &str) -> Result<Self, String> {
        use wasm_bindgen::closure::Closure;

        let socket = web_sys::WebSocket::new(url).map_err(|e| format!("{url}: {e:?}"))?;
        let (tx, events) = tokio::sync::mpsc::unbounded_channel();
        let open_tx = tx.clone();
        let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let _ = open_tx.send(SocketEvent::Open);
        });
        let message_tx = tx.clone();
        let on_message = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            if let Some(text) = event
                .dyn_ref::<web_sys::MessageEvent>()
                .and_then(|message| message.data().as_string())
            {
                let _ = message_tx.send(SocketEvent::Text(text));
            }
        });
        let on_close = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let reason = event
                .dyn_ref::<web_sys::CloseEvent>()
                .map_or_else(String::new, |close| format!("code {} {}", close.code(), close.reason()));
            let _ = tx.send(SocketEvent::Closed(reason));
        });
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let mut opened = Self { socket, events, _handlers: [on_open, on_message, on_close] };
        match opened.events.recv().await {
            Some(SocketEvent::Open) => Ok(opened),
            Some(SocketEvent::Closed(reason)) => Err(format!("{url}: closed before opening ({reason})")),
            _ => Err(format!("{url}: did not open")),
        }
    }

    /// The next text message, `None` once the socket closed.
    pub(crate) async fn next(&mut self) -> Option<String> {
        loop {
            match self.events.recv().await? {
                SocketEvent::Text(text) => return Some(text),
                SocketEvent::Open => {}
                SocketEvent::Closed(_) => return None,
            }
        }
    }
}

impl Drop for TextSocket {
    fn drop(&mut self) {
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

/// An HTTP request over `fetch`: the status and the body as text.
pub(crate) async fn fetch_text(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> Result<(u16, String), String> {
    use wasm_bindgen_futures::JsFuture;

    let error = |e: wasm_bindgen::JsValue| format!("{url}: {e:?}");
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    let request_headers = web_sys::Headers::new().map_err(error)?;
    for (name, value) in headers {
        request_headers.set(name, value).map_err(error)?;
    }
    init.set_headers(&request_headers);
    if let Some(body) = body {
        init.set_body(&wasm_bindgen::JsValue::from_str(&body));
    }
    let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(error)?;
    let window = web_sys::window().ok_or_else(|| format!("{url}: no window"))?;
    // A refused connection or a missing CORS header rejects with a TypeError.
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{url}: no answer ({e:?})"))?
        .dyn_into()
        .map_err(error)?;
    let text = JsFuture::from(response.text().map_err(error)?).await.map_err(error)?;
    Ok((response.status(), text.as_string().unwrap_or_default()))
}
