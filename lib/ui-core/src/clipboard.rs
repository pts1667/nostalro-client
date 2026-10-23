//! The system clipboard, read when the player pastes into a text field.

/// The clipboard's text, when it can be read right away.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Text that arrived after the paste that asked for it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_late_text() -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static LATE_TEXT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// A browser only hands the clipboard over asynchronously (and may ask the
/// player first), so the text is queued for [`take_late_text`].
#[cfg(target_arch = "wasm32")]
pub(crate) fn read_text() -> Option<String> {
    let clipboard = web_sys::window()?.navigator().clipboard();
    let promise = clipboard.read_text();
    wasm_bindgen_futures::spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(text) => {
                if let Some(text) = text.as_string() {
                    LATE_TEXT.with(|late| *late.borrow_mut() = Some(text));
                }
            }
            Err(e) => tracing::debug!("clipboard read refused: {e:?}"),
        }
    });
    None
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn take_late_text() -> Option<String> {
    LATE_TEXT.with(|late| late.borrow_mut().take())
}
