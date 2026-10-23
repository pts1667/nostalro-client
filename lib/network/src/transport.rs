//! The byte stream to a game server: a TCP socket on native, a WebSocket in a
//! browser. rust-ro accepts both on the same port (`server.enable_websocket`);
//! over WebSocket each binary message carries whole packets one way and stream
//! bytes the other, so the framing in [`crate::connection`] is the same.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::Transport;

#[cfg(target_arch = "wasm32")]
pub(crate) use web::Transport;
#[cfg(target_arch = "wasm32")]
pub use web::set_websocket_url_template;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

    pub(crate) struct Transport {
        reader: OwnedReadHalf,
        writer: OwnedWriteHalf,
    }

    impl Transport {
        pub(crate) async fn connect(addr: &str) -> io::Result<Self> {
            let stream = TcpStream::connect(addr).await?;
            let (reader, writer) = stream.into_split();
            Ok(Self { reader, writer })
        }

        pub(crate) async fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
            self.writer.write_all(data).await
        }

        /// Appends what arrived to `out`; 0 when the server closed the connection.
        pub(crate) async fn read_into(&mut self, out: &mut Vec<u8>) -> io::Result<usize> {
            let mut buf = [0u8; 4096];
            let n = self.reader.read(&mut buf).await?;
            out.extend_from_slice(&buf[..n]);
            Ok(n)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::io;
    use std::sync::Mutex;

    use tokio::sync::mpsc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

    /// Where `host:port` is reached, see [`set_websocket_url_template`].
    static URL_TEMPLATE: Mutex<Option<String>> = Mutex::new(None);

    /// Overrides the WebSocket URL of a game server, for a server behind a
    /// reverse proxy: `{host}`, `{port}` and `{addr}` (`host:port`) are
    /// replaced. Without one it is `ws://host:port/`, `wss://` on an https page.
    pub fn set_websocket_url_template(template: Option<String>) {
        *URL_TEMPLATE.lock().unwrap_or_else(|e| e.into_inner()) =
            template.filter(|t| !t.trim().is_empty());
    }

    fn websocket_url(addr: &str) -> String {
        let (host, port) = addr.rsplit_once(':').unwrap_or((addr, ""));
        if let Some(template) = URL_TEMPLATE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            return template
                .replace("{addr}", addr)
                .replace("{host}", host)
                .replace("{port}", port);
        }
        let secure = web_sys::window()
            .and_then(|w| w.location().protocol().ok())
            .is_some_and(|protocol| protocol == "https:");
        format!("{}://{addr}/", if secure { "wss" } else { "ws" })
    }

    enum Incoming {
        Open,
        Data(Vec<u8>),
        Closed(String),
    }

    pub(crate) struct Transport {
        socket: WebSocket,
        incoming: mpsc::UnboundedReceiver<Incoming>,
        _on_open: Closure<dyn FnMut(Event)>,
        _on_message: Closure<dyn FnMut(MessageEvent)>,
        _on_close: Closure<dyn FnMut(CloseEvent)>,
        _on_error: Closure<dyn FnMut(Event)>,
    }

    fn js_error(url: &str, e: wasm_bindgen::JsValue) -> io::Error {
        io::Error::other(format!("{url}: {e:?}"))
    }

    impl Transport {
        pub(crate) async fn connect(addr: &str) -> io::Result<Self> {
            let url = websocket_url(addr);
            let socket = WebSocket::new(&url).map_err(|e| js_error(&url, e))?;
            socket.set_binary_type(BinaryType::Arraybuffer);
            let (tx, incoming) = mpsc::unbounded_channel();

            let open_tx = tx.clone();
            let on_open = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
                let _ = open_tx.send(Incoming::Open);
            });
            let message_tx = tx.clone();
            let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                if let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let _ = message_tx.send(Incoming::Data(js_sys::Uint8Array::new(&buffer).to_vec()));
                }
            });
            let close_tx = tx.clone();
            let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
                let _ = close_tx.send(Incoming::Closed(format!(
                    "closed with code {} {}",
                    event.code(),
                    event.reason()
                )));
            });
            // The browser hides why a socket failed; a close event follows.
            let on_error = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
                let _ = tx.send(Incoming::Closed("WebSocket error".to_string()));
            });
            socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
            socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
            socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            // `transport` drops, closing the socket, on every error below.
            let mut transport = Self {
                socket,
                incoming,
                _on_open: on_open,
                _on_message: on_message,
                _on_close: on_close,
                _on_error: on_error,
            };
            match transport.incoming.recv().await {
                Some(Incoming::Open) => Ok(transport),
                Some(Incoming::Closed(reason)) => Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("{url}: {reason}"),
                )),
                Some(Incoming::Data(_)) | None => {
                    Err(io::Error::other(format!("{url}: unexpected WebSocket state")))
                }
            }
        }

        pub(crate) async fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
            self.socket
                .send_with_u8_array(data)
                .map_err(|e| js_error(&self.socket.url(), e))
        }

        /// Appends the next message to `out`; 0 once the socket closed.
        pub(crate) async fn read_into(&mut self, out: &mut Vec<u8>) -> io::Result<usize> {
            loop {
                match self.incoming.recv().await {
                    Some(Incoming::Data(bytes)) if !bytes.is_empty() => {
                        out.extend_from_slice(&bytes);
                        return Ok(bytes.len());
                    }
                    Some(Incoming::Data(_) | Incoming::Open) => {}
                    Some(Incoming::Closed(reason)) => {
                        tracing::info!("{}: {reason}", self.socket.url());
                        return Ok(0);
                    }
                    None => return Ok(0),
                }
            }
        }
    }

    impl Drop for Transport {
        fn drop(&mut self) {
            self.socket.set_onopen(None);
            self.socket.set_onmessage(None);
            self.socket.set_onclose(None);
            self.socket.set_onerror(None);
            let _ = self.socket.close();
        }
    }
}
