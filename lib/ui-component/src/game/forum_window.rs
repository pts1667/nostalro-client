//! In-game forum: the threads of the server's `/forum`, a thread with its
//! comments and the replies to them, and forms to start a thread and to comment.
//!
//! The window does no networking. It asks with [`GameEvent::RequestForum`] and
//! the client, which signs in as the playing character, hands the answer back
//! through [`ForumWindow::deliver`]. Every text is shown as plain text.

use web_time::{SystemTime, UNIX_EPOCH};

use crate::game::reference_window::{clipped_text, quad};
use crate::helper::scrollbar::{SCROLLBAR_W, ScrollbarIds, scrollbar};
use crate::helper::window_chrome::{
    draw_container, draw_footer, draw_sys_button, draw_titlebar, label_color, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::{ForumRequest, GameEvent};
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;
use ragnarok_ui::theme::{self, FallbackPalette as P};
use serde_json::Value;

pub const FORUM_WINDOW_ID: WidgetId = WidgetId(6400);
const CLOSE_BTN_ID: WidgetId = WidgetId(6401);
const BACK_BTN_ID: WidgetId = WidgetId(6402);
const REFRESH_BTN_ID: WidgetId = WidgetId(6403);
const NEW_THREAD_BTN_ID: WidgetId = WidgetId(6404);
const TITLE_INPUT_ID: WidgetId = WidgetId(6405);
const BODY_INPUT_ID: WidgetId = WidgetId(6406);
const POST_BTN_ID: WidgetId = WidgetId(6407);
const CANCEL_BTN_ID: WidgetId = WidgetId(6408);
const MORE_BTN_ID: WidgetId = WidgetId(6409);
const SCROLL_UP_ID: WidgetId = WidgetId(6410);
const SCROLL_DOWN_ID: WidgetId = WidgetId(6411);
const SCROLL_THUMB_ID: WidgetId = WidgetId(6412);
const COMMENT_INPUT_ID: WidgetId = WidgetId(6413);
const SEND_BTN_ID: WidgetId = WidgetId(6414);
const CANCEL_REPLY_BTN_ID: WidgetId = WidgetId(6415);
const RETRY_BTN_ID: WidgetId = WidgetId(6416);
const REPLIES_BTN_ID: WidgetId = WidgetId(6417);
const ROW_BASE_ID: u32 = 6420;
/// Clickable rows on show at once; the ids run up to the end of the block.
const ROW_SLOTS: u32 = 80;

const WIN_W: f32 = 480.0;
const TITLE_H: f32 = 17.0;
const TOOL_H: f32 = 26.0;
const ROW_H: f32 = 15.0;
/// Height of the area under the toolbar, in text rows.
const BODY_ROWS: usize = 20;
const BODY_H: f32 = ROW_H * BODY_ROWS as f32;
/// Space under the body so its border shows above the footer.
const WELL_GAP: f32 = 5.0;
const FOOTER_H: f32 = 26.0;
const WIN_H: f32 = TITLE_H + TOOL_H + BODY_H + WELL_GAP + FOOTER_H;
const PAD: f32 = 6.0;
const BASELINE: f32 = 12.0;
const INDENT_W: f32 = 14.0;
/// Replies deeper than this are drawn at this depth.
const MAX_INDENT: u8 = 6;
const SYS_BTN: f32 = 11.0;
const NAV_BTN_W: f32 = 22.0;
const BTN_H: f32 = 20.0;
const TEXT_BTN_W: f32 = 56.0;
const WIDE_BTN_W: f32 = 78.0;
/// A thread in the list: its title over its author and age.
const ITEM_H: f32 = ROW_H * 2.0;
/// The comment box under a thread: a line naming what it answers, then the box.
const COMPOSER_ROWS: usize = 5;
const COMPOSER_H: f32 = ROW_H * COMPOSER_ROWS as f32;
const LABEL_W: f32 = 40.0;

/// Threads asked for per page; the server allows up to 100.
pub const PAGE_SIZE: usize = 25;
/// The server's limits, in characters.
const TITLE_MAX: usize = 150;
const BODY_MAX: usize = 10_000;
const COMMENT_MAX: usize = 2_000;
/// `TextInput` counts bytes; a character takes up to four.
const UTF8_MAX: usize = 4;
/// Replies to the character kept for the Replies view, the newest first.
const NOTICES_KEPT: usize = 50;
/// More replies than this at once (the ones that came while away) make one
/// chat line instead of one each.
const NOTICE_LINES_MAX: usize = 3;
/// Characters of a reply quoted in its chat line.
const NOTICE_QUOTE_CHARS: usize = 60;

const LINK_COLOR: [f32; 4] = [0.10, 0.25, 0.75, 1.0];
const HOVER_ROW: [f32; 4] = [0.78, 0.86, 1.0, 0.7];
const MUTED_COLOR: [f32; 4] = [0.45, 0.47, 0.55, 1.0];
const ERROR_COLOR: [f32; 4] = [0.75, 0.12, 0.12, 1.0];
const GUIDE_COLOR: [f32; 4] = [0.70, 0.74, 0.84, 1.0];

#[derive(Clone, Debug, PartialEq)]
struct ThreadSummary {
    id: u64,
    author: String,
    title: String,
    body: String,
    created_at: i64,
    bumped: u64,
    comments: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct Comment {
    id: u64,
    author: String,
    body: String,
    created_at: i64,
    depth: u32,
}

/// Someone answered the character: a `reply` event of the forum's WebSocket.
#[derive(Clone, Debug, PartialEq)]
struct Notice {
    seq: u64,
    /// On a thread the character started, rather than on a comment it wrote.
    on_thread: bool,
    thread_id: u64,
    from: String,
    body: String,
    created_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
struct Thread {
    summary: ThreadSummary,
    comments: Vec<Comment>,
}

enum Load<T> {
    Pending,
    Ready(T),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    List,
    Thread(u64),
    Compose,
    /// The replies to the character that came this session.
    Replies,
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Info(String),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKind {
    Title,
    /// An author, with the age of the post after it.
    Meta,
    Body,
    Muted,
    Gap,
}

/// One text row of a thread on show.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    kind: RowKind,
    indent: u8,
    text: String,
    /// When the post was written; its age is drawn after `text`.
    time: Option<i64>,
    /// Drawn after the age (the comment count of the thread).
    extra: Option<String>,
    /// The comment a "Reply" on this row answers, and its author.
    reply: Option<(u64, String)>,
}

impl Row {
    fn new(kind: RowKind, indent: u8, text: String) -> Self {
        Self { kind, indent, text, time: None, extra: None, reply: None }
    }
}

/// A text box of several lines, and its text wrapped the last time it changed.
struct TextArea {
    input: TextInput,
    max_chars: usize,
    wrapped: Option<(String, f32, Vec<(usize, String)>)>,
}

impl TextArea {
    fn new(max_chars: usize) -> Self {
        Self { input: TextInput::new(max_chars * UTF8_MAX, false), max_chars, wrapped: None }
    }

    fn text(&self) -> &str {
        &self.input.text
    }

    fn clear(&mut self) {
        self.input.text.clear();
        self.input.cursor_pos = 0;
        self.input.clear_selection();
    }

    fn lines(&mut self, measure: &dyn Fn(&str) -> f32, width: f32) -> &[(usize, String)] {
        let stale = self
            .wrapped
            .as_ref()
            .is_none_or(|(text, w, _)| *text != self.input.text || *w != width);
        if stale {
            let lines = wrap_lines(measure, &self.input.text, width);
            self.wrapped = Some((self.input.text.clone(), width, lines));
        }
        &self.wrapped.as_ref().map(|(_, _, lines)| lines).unwrap()[..]
    }
}

pub struct ForumWindow {
    open: bool,
    view: View,
    signed_in_as: Option<String>,
    /// `None` until asked, and again once a post makes it stale.
    threads: Option<Load<Vec<ThreadSummary>>>,
    has_more: bool,
    loading_more: bool,
    thread: Option<(u64, Load<Thread>)>,
    /// The thread on show wrapped into rows; built on first draw.
    rows: Option<Vec<Row>>,
    /// Ask for the thread on show again without clearing it, after a comment.
    refresh_thread: bool,
    scroll_to_end: bool,
    scroll: usize,
    list_scroll: usize,
    /// Where the back button of a thread leads.
    back_to: View,
    notices: Vec<Notice>,
    unread: usize,
    title: TextInput,
    body: TextArea,
    comment: TextArea,
    reply_to: Option<(u64, String)>,
    /// The post waiting for the server's answer.
    posting: Option<ForumRequest>,
    status: Option<Status>,
    typing: bool,
}

impl Default for ForumWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ForumWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            view: View::List,
            signed_in_as: None,
            threads: None,
            has_more: false,
            loading_more: false,
            thread: None,
            rows: None,
            refresh_thread: false,
            scroll_to_end: false,
            scroll: 0,
            list_scroll: 0,
            back_to: View::List,
            notices: Vec::new(),
            unread: 0,
            title: TextInput::new(TITLE_MAX * UTF8_MAX, false),
            body: TextArea::new(BODY_MAX),
            comment: TextArea::new(COMMENT_MAX),
            reply_to: None,
            posting: None,
            status: None,
            typing: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether one of the window's text boxes takes the keys, so the game's
    /// hotkeys should leave them alone.
    pub fn is_typing(&self) -> bool {
        self.open && self.typing
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open && self.view == View::List {
            // The list is read again on each opening, so new threads show.
            self.threads = None;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// The client signed in as `name`.
    pub fn signed_in(&mut self, name: &str) {
        self.signed_in_as = Some(name.to_string());
    }

    /// Hands over the answer to a [`GameEvent::RequestForum`].
    pub fn deliver(&mut self, request: &ForumRequest, result: Result<Value, String>) {
        if self.posting.as_ref() == Some(request) {
            self.posting = None;
        }
        match request {
            ForumRequest::Threads { before: None, .. } => {
                self.loading_more = false;
                self.threads = Some(match result {
                    Ok(value) => {
                        let list = parse_threads(&value);
                        self.has_more = list.len() >= PAGE_SIZE;
                        Load::Ready(list)
                    }
                    Err(message) => Load::Failed(message),
                });
            }
            ForumRequest::Threads { before: Some(_), .. } => {
                self.loading_more = false;
                match result {
                    Ok(value) => {
                        let more = parse_threads(&value);
                        self.has_more = more.len() >= PAGE_SIZE;
                        if let Some(Load::Ready(list)) = &mut self.threads {
                            for thread in more {
                                if !list.iter().any(|t| t.id == thread.id) {
                                    list.push(thread);
                                }
                            }
                        }
                    }
                    Err(message) => self.status = Some(Status::Error(message)),
                }
            }
            ForumRequest::Thread { id } => {
                if self.thread.as_ref().is_some_and(|(current, _)| current == id) {
                    let load = match result {
                        Ok(value) => parse_thread(&value)
                            .map(Load::Ready)
                            .unwrap_or_else(|| Load::Failed("unreadable thread".to_string())),
                        Err(message) => Load::Failed(message),
                    };
                    self.thread = Some((*id, load));
                    self.rows = None;
                }
            }
            ForumRequest::CreateThread { .. } => match result {
                Ok(value) => {
                    self.title.text.clear();
                    self.title.cursor_pos = 0;
                    self.body.clear();
                    self.threads = None;
                    if let Some(id) = value.get("id").and_then(Value::as_u64) {
                        self.open_thread(id);
                    } else {
                        self.view = View::List;
                    }
                    self.status = Some(Status::Info(rust_i18n::t!("forum.thread_posted").into_owned()));
                }
                Err(message) => self.status = Some(Status::Error(message)),
            },
            ForumRequest::Comment { thread_id, .. } => match result {
                Ok(_) => {
                    self.comment.clear();
                    self.reply_to = None;
                    self.threads = None;
                    if self.view == View::Thread(*thread_id) {
                        self.refresh_thread = true;
                        self.scroll_to_end = true;
                    }
                    self.status = Some(Status::Info(rust_i18n::t!("forum.comment_posted").into_owned()));
                }
                Err(message) => self.status = Some(Status::Error(message)),
            },
        }
    }

    /// Takes the replies to the character that the client received, and
    /// answers the chat lines that tell the player about them.
    pub fn add_notices(&mut self, values: &[Value]) -> Vec<String> {
        let mut fresh: Vec<Notice> = values.iter().filter_map(parse_notice).collect();
        fresh.retain(|n| !self.notices.iter().any(|kept| kept.seq == n.seq));
        if fresh.is_empty() {
            return Vec::new();
        }
        if fresh.iter().any(|n| self.view == View::Thread(n.thread_id)) {
            self.refresh_thread = true;
        }
        let lines = if fresh.len() > NOTICE_LINES_MAX {
            vec![rust_i18n::t!("forum.notice_many", count = fresh.len()).into_owned()]
        } else {
            fresh.iter().map(notice_line).collect()
        };
        if !(self.open && self.view == View::Replies) {
            self.unread = (self.unread + fresh.len()).min(NOTICES_KEPT);
        }
        fresh.sort_by_key(|n| std::cmp::Reverse(n.seq));
        fresh.append(&mut self.notices);
        fresh.truncate(NOTICES_KEPT);
        self.notices = fresh;
        lines
    }

    /// Leaves whatever view is on show for the list, or a thread for the view
    /// it was opened from.
    fn go_back(&mut self) {
        let target = match self.view {
            View::Thread(_) => self.back_to,
            View::List | View::Compose | View::Replies => View::List,
        };
        self.view = target;
        self.scroll = if target == View::List { self.list_scroll } else { 0 };
        self.status = None;
    }

    fn leave_list(&mut self) {
        if self.view == View::List {
            self.list_scroll = self.scroll;
        }
    }

    fn open_replies(&mut self) {
        self.leave_list();
        self.view = View::Replies;
        self.scroll = 0;
        self.status = None;
        self.unread = 0;
    }

    fn open_thread(&mut self, id: u64) {
        self.leave_list();
        if matches!(self.view, View::List | View::Replies) {
            self.back_to = self.view;
        }
        self.view = View::Thread(id);
        self.status = None;
        self.scroll = 0;
        self.reply_to = None;
        if self.thread.as_ref().is_none_or(|(current, _)| *current != id) {
            self.thread = None;
            self.rows = None;
        }
    }

    fn start_thread(&mut self) {
        self.leave_list();
        self.view = View::Compose;
        self.status = None;
    }

    fn post(&mut self, request: ForumRequest, events: &mut Vec<GameEvent>) {
        self.posting = Some(request.clone());
        self.status = Some(Status::Info(rust_i18n::t!("forum.posting").into_owned()));
        events.push(GameEvent::RequestForum(request));
    }

    fn signed_in_text(&self) -> String {
        match &self.signed_in_as {
            Some(name) => rust_i18n::t!("forum.signed_in_as", name = name.as_str()).into_owned(),
            None => String::new(),
        }
    }

    fn draw_toolbar(&mut self, ui: &mut UiFrame, x: f32, y: f32, events: &mut Vec<GameEvent>) {
        let by = y + (TOOL_H - BTN_H) / 2.0;
        let mut bx = x + PAD;
        if self.view != View::List {
            if ui.button(BACK_BTN_ID, Rect::new(bx, by, NAV_BTN_W, BTN_H), "<").clicked() {
                self.go_back();
                return;
            }
            bx += NAV_BTN_W + 3.0;
        }
        match self.view {
            View::List => {
                let refresh = Rect::new(bx, by, TEXT_BTN_W, BTN_H);
                if ui.button(REFRESH_BTN_ID, refresh, &rust_i18n::t!("forum.refresh")).clicked() {
                    self.threads = None;
                    self.status = None;
                }
                bx += TEXT_BTN_W + 3.0;
                let new = Rect::new(bx, by, WIDE_BTN_W, BTN_H);
                if ui.button(NEW_THREAD_BTN_ID, new, &rust_i18n::t!("forum.new_thread")).clicked() {
                    self.start_thread();
                }
                bx += WIDE_BTN_W + 3.0;
                let label = if self.unread > 0 {
                    rust_i18n::t!("forum.replies_unread", count = self.unread)
                } else {
                    rust_i18n::t!("forum.replies")
                };
                let replies = Rect::new(bx, by, WIDE_BTN_W, BTN_H);
                if ui.button(REPLIES_BTN_ID, replies, &label).clicked() {
                    self.open_replies();
                }
            }
            View::Thread(id) => {
                let refresh = Rect::new(bx, by, TEXT_BTN_W, BTN_H);
                if ui.button(REFRESH_BTN_ID, refresh, &rust_i18n::t!("forum.refresh")).clicked() {
                    self.thread = Some((id, Load::Pending));
                    self.rows = None;
                    self.status = None;
                    events.push(GameEvent::RequestForum(ForumRequest::Thread { id }));
                }
            }
            View::Compose => {
                ui.text(bx + 2.0, by + BASELINE + 2.0, &rust_i18n::t!("forum.new_thread"), label_color());
            }
            View::Replies => {
                ui.text(bx + 2.0, by + BASELINE + 2.0, &rust_i18n::t!("forum.replies_title"), label_color());
            }
        }
        let who = self.signed_in_text();
        let right = x + WIN_W - PAD;
        let w = ui.atlas.measure_text(&who);
        ui.text(right - w, by + BASELINE + 2.0, &who, MUTED_COLOR);
    }

    fn build_list(&mut self, ui: &mut UiFrame, well: Rect, now: i64, events: &mut Vec<GameEvent>) {
        if self.threads.is_none() {
            self.threads = Some(Load::Pending);
            self.has_more = false;
            events.push(GameEvent::RequestForum(ForumRequest::Threads { before: None, limit: PAGE_SIZE }));
        }
        draw_well(ui, well);
        let text_x = well.x + 6.0;
        let first_baseline = well.y + BASELINE + 2.0;
        let mut open = None;
        let mut retry = false;
        match &self.threads {
            Some(Load::Ready(list)) if list.is_empty() => {
                ui.text(text_x, first_baseline, &rust_i18n::t!("forum.no_threads"), MUTED_COLOR);
            }
            Some(Load::Ready(list)) => {
                let fit = ((well.h / ITEM_H).floor() as usize).max(1);
                let max_scroll = list.len().saturating_sub(fit);
                let content = Rect::new(well.x, well.y, well.w - SCROLLBAR_W, well.h);
                self.scroll = scrollbar(
                    ui,
                    ScrollbarIds { up: SCROLL_UP_ID, down: SCROLL_DOWN_ID, thumb: SCROLL_THUMB_ID },
                    self.scroll,
                    fit,
                    max_scroll,
                    content,
                    well.x + well.w - SCROLLBAR_W,
                    well.y,
                    well.h,
                );
                for (slot, thread) in list.iter().skip(self.scroll).take(fit).enumerate() {
                    let row = Rect::new(content.x + 1.0, well.y + slot as f32 * ITEM_H, content.w - 2.0, ITEM_H);
                    if draw_thread_item(ui, WidgetId(ROW_BASE_ID + slot as u32), row, thread, now) {
                        open = Some(thread.id);
                    }
                }
            }
            Some(Load::Failed(message)) => {
                retry = draw_failure(ui, well, message);
            }
            Some(Load::Pending) | None => {
                ui.text(text_x, first_baseline, &rust_i18n::t!("forum.loading"), MUTED_COLOR);
            }
        }
        if retry {
            self.threads = None;
        }
        if let Some(id) = open {
            self.open_thread(id);
        }
    }

    fn build_replies(&mut self, ui: &mut UiFrame, well: Rect, now: i64) {
        draw_well(ui, well);
        self.unread = 0;
        if self.notices.is_empty() {
            ui.text(well.x + 6.0, well.y + BASELINE + 2.0, &rust_i18n::t!("forum.no_replies"), MUTED_COLOR);
            return;
        }
        let fit = ((well.h / ITEM_H).floor() as usize).max(1);
        let max_scroll = self.notices.len().saturating_sub(fit);
        let content = Rect::new(well.x, well.y, well.w - SCROLLBAR_W, well.h);
        self.scroll = scrollbar(
            ui,
            ScrollbarIds { up: SCROLL_UP_ID, down: SCROLL_DOWN_ID, thumb: SCROLL_THUMB_ID },
            self.scroll,
            fit,
            max_scroll,
            content,
            well.x + well.w - SCROLLBAR_W,
            well.y,
            well.h,
        );
        let mut open = None;
        for (slot, notice) in self.notices.iter().skip(self.scroll).take(fit).enumerate() {
            let row = Rect::new(content.x + 1.0, well.y + slot as f32 * ITEM_H, content.w - 2.0, ITEM_H);
            if draw_notice_item(ui, WidgetId(ROW_BASE_ID + slot as u32), row, notice, now) {
                open = Some(notice.thread_id);
            }
        }
        if let Some(id) = open {
            self.open_thread(id);
        }
    }

    fn build_thread(
        &mut self,
        ui: &mut UiFrame,
        well: Rect,
        id: u64,
        now: i64,
        events: &mut Vec<GameEvent>,
    ) {
        if self.thread.as_ref().is_none_or(|(current, _)| *current != id) {
            self.thread = Some((id, Load::Pending));
            self.rows = None;
            events.push(GameEvent::RequestForum(ForumRequest::Thread { id }));
        } else if self.refresh_thread {
            events.push(GameEvent::RequestForum(ForumRequest::Thread { id }));
        }
        self.refresh_thread = false;

        let list = Rect::new(well.x, well.y, well.w, well.h - COMPOSER_H);
        draw_well(ui, list);
        let mut retry = false;
        let mut ready = false;
        match &self.thread {
            Some((_, Load::Ready(thread))) => {
                ready = true;
                let width = list.w - SCROLLBAR_W - 8.0;
                if self.rows.is_none() {
                    self.rows = Some(thread_rows(&|t: &str| ui.atlas.measure_text(t), thread, width));
                }
            }
            Some((_, Load::Failed(message))) => retry = draw_failure(ui, list, message),
            Some((_, Load::Pending)) | None => {
                ui.text(list.x + 6.0, list.y + BASELINE + 2.0, &rust_i18n::t!("forum.loading"), MUTED_COLOR);
            }
        }
        if retry {
            self.thread = None;
            return;
        }
        if !ready {
            return;
        }
        // Taken out so the rows can be drawn while `self` is borrowed mutably.
        let rows = self.rows.take().unwrap_or_default();
        self.draw_thread_rows(ui, &rows, list, now);
        self.rows = Some(rows);

        let composer = Rect::new(well.x, list.y + list.h + 4.0, well.w, COMPOSER_H - 4.0);
        self.draw_composer(ui, composer, id, events);
    }

    fn draw_thread_rows(&mut self, ui: &mut UiFrame, rows: &[Row], list: Rect, now: i64) {
        let fit = ((list.h / ROW_H).floor() as usize).max(1);
        let max_scroll = rows.len().saturating_sub(fit);
        if std::mem::take(&mut self.scroll_to_end) {
            self.scroll = max_scroll;
        }
        let content = Rect::new(list.x, list.y, list.w - SCROLLBAR_W, list.h);
        self.scroll = scrollbar(
            ui,
            ScrollbarIds { up: SCROLL_UP_ID, down: SCROLL_DOWN_ID, thumb: SCROLL_THUMB_ID },
            self.scroll,
            fit,
            max_scroll,
            content,
            list.x + list.w - SCROLLBAR_W,
            list.y,
            list.h,
        );
        let mut slot = 0u32;
        for (n, row) in rows.iter().skip(self.scroll).take(fit).enumerate() {
            let top = list.y + n as f32 * ROW_H;
            let rect = Rect::new(content.x + 1.0, top, content.w - 2.0, ROW_H);
            if let Some(reply) = draw_row(ui, row, rect, &mut slot, now) {
                self.reply_to = Some(reply);
                ui.set_focus(COMMENT_INPUT_ID);
            }
        }
    }

    fn draw_composer(&mut self, ui: &mut UiFrame, area: Rect, thread_id: u64, events: &mut Vec<GameEvent>) {
        let baseline = area.y + BASELINE;
        let send = Rect::new(area.x + area.w - TEXT_BTN_W, area.y + area.h - BTN_H, TEXT_BTN_W, BTN_H);
        match &self.reply_to {
            Some((_, author)) => {
                let text = rust_i18n::t!("forum.replying_to", name = author.as_str());
                ui.text(area.x + 2.0, baseline, &text, label_color());
                let w = ui.atlas.measure_text(&text);
                let cancel = Rect::new(area.x + 8.0 + w, area.y + 1.0, SYS_BTN + 2.0, SYS_BTN + 2.0);
                let resp = ui.interact(CANCEL_REPLY_BTN_ID, cancel);
                if resp.hovered() {
                    ui.any_interactive_hovered = true;
                }
                draw_sys_button(ui, cancel, (cancel.w, cancel.h), resp.hovered(), Some('x'));
                if resp.clicked() {
                    self.reply_to = None;
                }
            }
            None => {
                ui.text(area.x + 2.0, baseline, &rust_i18n::t!("forum.comment_hint"), MUTED_COLOR);
            }
        }
        let boxed = Rect::new(area.x, area.y + ROW_H, send.x - area.x - 4.0, area.h - ROW_H);
        let (focused, submitted) = text_area(ui, COMMENT_INPUT_ID, boxed, &mut self.comment, true);
        self.typing |= focused;
        let clicked = ui.button(SEND_BTN_ID, send, &rust_i18n::t!("forum.send")).clicked();
        if (clicked || submitted) && self.posting.is_none() {
            let body = self.comment.text().trim().to_string();
            if let Some(problem) = check_text(&body, self.comment.max_chars) {
                self.status = Some(Status::Error(problem));
            } else {
                let parent = self.reply_to.as_ref().map(|(id, _)| *id);
                self.post(ForumRequest::Comment { thread_id, body, parent }, events);
            }
        }
    }

    fn build_compose(&mut self, ui: &mut UiFrame, well: Rect) {
        let input_h = 18.0;
        ui.text(well.x, well.y + 13.0, &rust_i18n::t!("forum.subject"), label_color());
        let title = Rect::new(well.x + LABEL_W, well.y, well.w - LABEL_W, input_h);
        let resp = ui.text_input(TITLE_INPUT_ID, title, &mut self.title, TextInputBg::Default);
        self.typing |= resp.has_focus();

        let body_top = well.y + input_h + 8.0;
        ui.text(well.x, body_top + 13.0, &rust_i18n::t!("forum.text"), label_color());
        let body = Rect::new(well.x + LABEL_W, body_top, well.w - LABEL_W, well.y + well.h - body_top);
        let (focused, _) = text_area(ui, BODY_INPUT_ID, body, &mut self.body, false);
        self.typing |= focused;
    }

    fn draw_footer_row(&mut self, ui: &mut UiFrame, x: f32, y: f32, events: &mut Vec<GameEvent>) {
        let by = y + (FOOTER_H - BTN_H) / 2.0;
        let right = x + WIN_W - PAD;
        let mut buttons_left = right;
        match self.view {
            View::List => {
                let more_ready = matches!(&self.threads, Some(Load::Ready(list)) if !list.is_empty());
                if more_ready && self.has_more {
                    let more = Rect::new(right - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
                    buttons_left = more.x;
                    let label = if self.loading_more { rust_i18n::t!("forum.loading") } else { rust_i18n::t!("forum.more") };
                    if ui.button(MORE_BTN_ID, more, &label).clicked() && !self.loading_more {
                        let before = match &self.threads {
                            Some(Load::Ready(list)) => list.last().map(|t| t.bumped),
                            _ => None,
                        };
                        if let Some(before) = before {
                            self.loading_more = true;
                            events.push(GameEvent::RequestForum(ForumRequest::Threads {
                                before: Some(before),
                                limit: PAGE_SIZE,
                            }));
                        }
                    }
                }
            }
            View::Compose => {
                let post = Rect::new(right - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
                let cancel = Rect::new(post.x - 3.0 - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
                buttons_left = cancel.x;
                if ui.button(CANCEL_BTN_ID, cancel, &rust_i18n::t!("forum.cancel")).clicked() {
                    self.go_back();
                } else if ui.button(POST_BTN_ID, post, &rust_i18n::t!("forum.post")).clicked()
                    && self.posting.is_none()
                {
                    let title = self.title.text.trim().to_string();
                    let body = self.body.text().trim().to_string();
                    let problem = check_text(&title, TITLE_MAX)
                        .map(|p| format!("{}: {p}", rust_i18n::t!("forum.subject_name")))
                        .or_else(|| check_text(&body, BODY_MAX));
                    match problem {
                        Some(problem) => self.status = Some(Status::Error(problem)),
                        None => self.post(ForumRequest::CreateThread { title, body }, events),
                    }
                }
            }
            View::Thread(_) | View::Replies => {}
        }
        let (text, color) = match &self.status {
            Some(Status::Info(text)) => (text.clone(), text_color()),
            Some(Status::Error(text)) => (text.clone(), ERROR_COLOR),
            None => match (&self.view, &self.threads) {
                (View::List, Some(Load::Ready(list))) if !list.is_empty() => {
                    (rust_i18n::t!("forum.thread_count", count = list.len()).into_owned(), MUTED_COLOR)
                }
                _ => (String::new(), MUTED_COLOR),
            },
        };
        clipped_text(ui, x + PAD, y + FOOTER_H / 2.0 + 4.0, &text, color, buttons_left - 4.0);
    }
}

impl Window for ForumWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for ForumWindow {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.is_typing()
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.typing = false;
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();
        let now = unix_now();

        let win = ui.window_at(FORUM_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 160.0, 110.0);
        let (x, y) = (win.x, win.y);
        ui.interact(FORUM_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + 13.0, &rust_i18n::t!("forum.title"), text_color());
        let close = Rect::new(x + WIN_W - 3.0 - SYS_BTN, y + 3.0, SYS_BTN, SYS_BTN);
        let close_resp = ui.interact(CLOSE_BTN_ID, close);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(ui, close, (SYS_BTN, SYS_BTN), close_resp.hovered(), Some('x'));
        if close_resp.clicked() {
            self.open = false;
            return events;
        }

        let body_y = y + TITLE_H;
        draw_container(ui, x, body_y, WIN_W, TOOL_H + BODY_H + WELL_GAP);
        self.draw_toolbar(ui, x, body_y, &mut events);

        let well = Rect::new(x + PAD, body_y + TOOL_H, WIN_W - PAD * 2.0, BODY_H);
        match self.view {
            View::List => self.build_list(ui, well, now, &mut events),
            View::Thread(id) => self.build_thread(ui, well, id, now, &mut events),
            View::Compose => self.build_compose(ui, well),
            View::Replies => self.build_replies(ui, well, now),
        }

        let footer_y = body_y + TOOL_H + BODY_H + WELL_GAP;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        self.draw_footer_row(ui, x, footer_y, &mut events);
        events
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn draw_well(ui: &mut UiFrame, well: Rect) {
    quad(ui, well.x - 1.0, well.y - 1.0, well.w + 2.0, well.h + 2.0, P::WIN_BORDER);
    quad(ui, well.x, well.y, well.w, well.h, P::SLOT_FILL);
}

/// The error of a failed load and a button to try again; answers the button.
fn draw_failure(ui: &mut UiFrame, well: Rect, message: &str) -> bool {
    let shown = rust_i18n::t!("forum.error", message = message);
    clipped_text(ui, well.x + 6.0, well.y + BASELINE + 2.0, &shown, ERROR_COLOR, well.x + well.w - 4.0);
    let retry = Rect::new(well.x + 6.0, well.y + 26.0, TEXT_BTN_W, BTN_H);
    ui.button(RETRY_BTN_ID, retry, &rust_i18n::t!("forum.retry")).clicked()
}

fn bold_text(ui: &mut UiFrame, x: f32, y: f32, text: &str, color: [f32; 4], right: f32) {
    clipped_text(ui, x, y, text, color, right);
    clipped_text(ui, x + 1.0, y, text, color, right);
}

/// One thread of the list; answers whether it was clicked.
fn draw_thread_item(ui: &mut UiFrame, id: WidgetId, row: Rect, thread: &ThreadSummary, now: i64) -> bool {
    let resp = ui.interact(id, row);
    if resp.hovered() {
        ui.any_interactive_hovered = true;
        quad(ui, row.x, row.y, row.w, row.h - 1.0, HOVER_ROW);
    }
    let right = row.x + row.w - 4.0;
    let count = comments_label(thread.comments);
    let count_w = ui.atlas.measure_text(&count);
    ui.text(right - count_w, row.y + BASELINE + 1.0, &count, MUTED_COLOR);
    bold_text(ui, row.x + 4.0, row.y + BASELINE + 1.0, &thread.title, LINK_COLOR, right - count_w - 8.0);
    let meta = format!("{} · {}", thread.author, age(now, thread.created_at));
    clipped_text(ui, row.x + 4.0, row.y + ROW_H + BASELINE, &meta, MUTED_COLOR, right);
    quad(ui, row.x, row.y + row.h - 1.0, row.w, 1.0, GUIDE_COLOR);
    resp.clicked()
}

/// One reply to the character; answers whether it was clicked.
fn draw_notice_item(ui: &mut UiFrame, id: WidgetId, row: Rect, notice: &Notice, now: i64) -> bool {
    let resp = ui.interact(id, row);
    if resp.hovered() {
        ui.any_interactive_hovered = true;
        quad(ui, row.x, row.y, row.w, row.h - 1.0, HOVER_ROW);
    }
    let right = row.x + row.w - 4.0;
    let when = age(now, notice.created_at);
    let when_w = ui.atlas.measure_text(&when);
    ui.text(right - when_w, row.y + BASELINE + 1.0, &when, MUTED_COLOR);
    let what = notice_heading(notice);
    bold_text(ui, row.x + 4.0, row.y + BASELINE + 1.0, &what, LINK_COLOR, right - when_w - 8.0);
    let quote = one_line(&notice.body, usize::MAX);
    clipped_text(ui, row.x + 4.0, row.y + ROW_H + BASELINE, &quote, MUTED_COLOR, right);
    quad(ui, row.x, row.y + row.h - 1.0, row.w, 1.0, GUIDE_COLOR);
    resp.clicked()
}

/// "Bob replied to your thread".
fn notice_heading(notice: &Notice) -> String {
    let name = notice.from.as_str();
    if notice.on_thread {
        rust_i18n::t!("forum.replied_to_thread", name = name).into_owned()
    } else {
        rust_i18n::t!("forum.replied_to_comment", name = name).into_owned()
    }
}

/// The chat line telling the player about a reply.
fn notice_line(notice: &Notice) -> String {
    let quote = one_line(&notice.body, NOTICE_QUOTE_CHARS);
    rust_i18n::t!("forum.notice", what = notice_heading(notice), quote = quote).into_owned()
}

/// `text` on one line, cut to `max` characters with an ellipsis.
fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let cut: String = flat.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", cut.trim_end())
    }
}

/// One row of a thread; answers the comment its "Reply" was clicked for.
fn draw_row(ui: &mut UiFrame, row: &Row, rect: Rect, slot: &mut u32, now: i64) -> Option<(u64, String)> {
    let indent = row.indent.min(MAX_INDENT);
    for level in 1..=indent {
        let gx = rect.x + 4.0 + (f32::from(level) - 0.5) * INDENT_W;
        quad(ui, gx, rect.y, 1.0, rect.h, GUIDE_COLOR);
    }
    let x = rect.x + 4.0 + f32::from(indent) * INDENT_W;
    let baseline = rect.y + BASELINE;
    let mut right = rect.x + rect.w - 3.0;
    let mut clicked = None;
    if let Some(reply) = &row.reply
        && *slot < ROW_SLOTS
    {
        let label = rust_i18n::t!("forum.reply");
        let w = ui.atlas.measure_text(&label);
        let hit = Rect::new(right - w - 4.0, rect.y, w + 4.0, rect.h);
        let resp = ui.interact(WidgetId(ROW_BASE_ID + *slot), hit);
        *slot += 1;
        if resp.hovered() {
            ui.any_interactive_hovered = true;
            quad(ui, right - w, baseline + 2.0, w, 1.0, LINK_COLOR);
        }
        ui.text(right - w, baseline, &label, LINK_COLOR);
        if resp.clicked() {
            clicked = Some(reply.clone());
        }
        right = hit.x - 4.0;
    }
    match row.kind {
        RowKind::Title => bold_text(ui, x, baseline, &row.text, label_color(), right),
        RowKind::Meta => {
            bold_text(ui, x, baseline, &row.text, label_color(), right);
            let mut after = String::new();
            if let Some(time) = row.time {
                after.push_str(&format!(" · {}", age(now, time)));
            }
            if let Some(extra) = &row.extra {
                after.push_str(&format!(" · {extra}"));
            }
            let ax = x + ui.atlas.measure_text(&row.text) + 1.0;
            clipped_text(ui, ax, baseline, &after, MUTED_COLOR, right);
        }
        RowKind::Body => clipped_text(ui, x, baseline, &row.text, text_color(), right),
        RowKind::Muted => clipped_text(ui, x, baseline, &row.text, MUTED_COLOR, right),
        RowKind::Gap => {}
    }
    clicked
}

/// A text box of several lines. Enter adds a line break, or asks to send when
/// `enter_sends` and Shift is up. Answers whether the box has the keys and
/// whether it asked to send.
fn text_area(ui: &mut UiFrame, id: WidgetId, rect: Rect, area: &mut TextArea, enter_sends: bool) -> (bool, bool) {
    let resp = ui.interact(id, rect);
    if resp.hovered() {
        ui.any_interactive_hovered = true;
    }
    let focused = resp.has_focus();
    theme::fallback_text_input(ui, rect, focused);
    let line_h = ROW_H;
    let width = rect.w - 8.0;
    let visible = (((rect.h - 4.0) / line_h).floor() as usize).max(1);

    let mut submitted = false;
    if focused {
        let measure = |t: &str| ui.atlas.measure_text(t);
        if ui.ctx.key_up || ui.ctx.key_down {
            let lines = area.lines(&measure, width).to_vec();
            let (line, col_x) = cursor_place(&measure, &lines, area.input.cursor_pos);
            let target = if ui.ctx.key_up { line.checked_sub(1) } else { Some(line + 1).filter(|l| *l < lines.len()) };
            if let Some(target) = target {
                area.input.cursor_pos = index_at(&measure, &lines, target, col_x);
            }
        }
        area.input.process_keys(ui.ctx);
        if ui.ctx.key_enter {
            if enter_sends && !ui.ctx.shift_pressed {
                submitted = true;
            } else if area.input.text.len() < area.input.max_len {
                let at = byte_offset(&area.input.text, area.input.cursor_pos);
                area.input.text.insert(at, '\n');
                area.input.cursor_pos += 1;
            }
        }
    }

    let measure = |t: &str| ui.atlas.measure_text(t);
    let lines = area.lines(&measure, width).to_vec();
    let (cursor_line, cursor_x) = cursor_place(&measure, &lines, area.input.cursor_pos);
    let first = (cursor_line + 1).saturating_sub(visible);
    if resp.clicked() {
        let line = first + ((ui.ctx.mouse_y - rect.y - 2.0) / line_h).max(0.0) as usize;
        let line = line.min(lines.len().saturating_sub(1));
        area.input.cursor_pos = index_at(&measure, &lines, line, ui.ctx.mouse_x - rect.x - 4.0);
        area.input.clear_selection();
    }
    for (n, (_, text)) in lines.iter().skip(first).take(visible).enumerate() {
        let baseline = rect.y + 2.0 + n as f32 * line_h + BASELINE;
        clipped_text(ui, rect.x + 4.0, baseline, text, text_color(), rect.x + rect.w - 4.0);
    }
    if focused && (ui.elapsed_secs % 1.0) < 0.5 && cursor_line >= first && cursor_line < first + visible {
        let cy = rect.y + 4.0 + (cursor_line - first) as f32 * line_h;
        quad(ui, rect.x + 4.0 + cursor_x, cy, 1.0, 12.0, text_color());
    }
    (focused, submitted)
}

fn byte_offset(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map_or(text.len(), |(i, _)| i)
}

/// The line the cursor at `cursor` (in characters) is on, and its distance from
/// the start of that line.
fn cursor_place(measure: &dyn Fn(&str) -> f32, lines: &[(usize, String)], cursor: usize) -> (usize, f32) {
    let line = lines.iter().rposition(|(start, _)| *start <= cursor).unwrap_or(0);
    let Some((start, text)) = lines.get(line) else {
        return (0, 0.0);
    };
    let before: String = text.chars().take(cursor - start).collect();
    (line, measure(&before))
}

/// The character index nearest to `x` pixels into line `line`.
fn index_at(measure: &dyn Fn(&str) -> f32, lines: &[(usize, String)], line: usize, x: f32) -> usize {
    let Some((start, text)) = lines.get(line) else {
        return 0;
    };
    let chars: Vec<char> = text.chars().filter(|c| *c != '\n').collect();
    // A line broken at a space ends before that space, so the cursor stays on it.
    let usable = if lines.get(line + 1).is_some() && chars.last() == Some(&' ') { chars.len() - 1 } else { chars.len() };
    let mut best = 0;
    let mut prefix = String::new();
    for (n, c) in chars.iter().take(usable).enumerate() {
        let before = measure(&prefix);
        prefix.push(*c);
        let after = measure(&prefix);
        if x < (before + after) / 2.0 {
            break;
        }
        best = n + 1;
    }
    start + best
}

/// Lines of `text` no wider than `max_w`: broken after the last space that
/// fits, inside a word where none does, and at each line break. Every line
/// comes with the index of its first character so a cursor can be placed; the
/// characters are kept as typed, spaces included.
fn wrap_lines(measure: &dyn Fn(&str) -> f32, text: &str, max_w: f32) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\n' {
            lines.push((start, chars[start..i].iter().collect()));
            start = i + 1;
            i += 1;
            continue;
        }
        let line: String = chars[start..=i].iter().collect();
        if i > start && measure(&line) > max_w {
            let space = chars[start..i].iter().rposition(|c| *c == ' ').map(|p| start + p);
            let cut = match space {
                Some(space) if space > start => space + 1,
                _ => i,
            };
            lines.push((start, chars[start..cut].iter().collect()));
            start = cut;
            continue;
        }
        i += 1;
    }
    lines.push((start, chars[start..].iter().collect()));
    lines
}

/// Rows of the thread on show: its title, author and text, then each comment
/// under the one it answers.
fn thread_rows(measure: &dyn Fn(&str) -> f32, thread: &Thread, width: f32) -> Vec<Row> {
    let mut rows = Vec::new();
    let summary = &thread.summary;
    for (_, line) in wrap_lines(measure, &summary.title, width) {
        rows.push(Row::new(RowKind::Title, 0, line));
    }
    let mut meta = Row::new(RowKind::Meta, 0, summary.author.clone());
    meta.time = Some(summary.created_at);
    meta.extra = Some(comments_label(summary.comments));
    rows.push(meta);
    for (_, line) in wrap_lines(measure, &summary.body, width) {
        rows.push(Row::new(RowKind::Body, 0, line));
    }
    rows.push(Row::new(RowKind::Gap, 0, String::new()));
    if thread.comments.is_empty() {
        rows.push(Row::new(RowKind::Muted, 0, rust_i18n::t!("forum.no_comments").into_owned()));
    }
    for comment in &thread.comments {
        let indent = u8::try_from(comment.depth).unwrap_or(u8::MAX).min(MAX_INDENT);
        let mut meta = Row::new(RowKind::Meta, indent, comment.author.clone());
        meta.time = Some(comment.created_at);
        meta.reply = Some((comment.id, comment.author.clone()));
        rows.push(meta);
        let inner = (width - f32::from(indent) * INDENT_W).max(80.0);
        for (_, line) in wrap_lines(measure, &comment.body, inner) {
            rows.push(Row::new(RowKind::Body, indent, line));
        }
        rows.push(Row::new(RowKind::Gap, indent, String::new()));
    }
    rows
}

/// What the server would refuse in a text, said before sending it.
fn check_text(text: &str, max_chars: usize) -> Option<String> {
    let count = text.chars().count();
    if count == 0 {
        Some(rust_i18n::t!("forum.empty").into_owned())
    } else if count > max_chars {
        Some(rust_i18n::t!("forum.too_long", count = count, max = max_chars).into_owned())
    } else {
        None
    }
}

fn comments_label(count: u64) -> String {
    if count == 1 {
        rust_i18n::t!("forum.one_comment").into_owned()
    } else {
        rust_i18n::t!("forum.comments", count = count).into_owned()
    }
}

/// How long ago `then` was, both in Unix seconds.
fn age(now: i64, then: i64) -> String {
    let secs = (now - then).max(0);
    match secs {
        0..60 => rust_i18n::t!("forum.just_now").into_owned(),
        60..3600 => rust_i18n::t!("forum.minutes_ago", count = secs / 60).into_owned(),
        3600..86400 => rust_i18n::t!("forum.hours_ago", count = secs / 3600).into_owned(),
        _ => rust_i18n::t!("forum.days_ago", count = secs / 86400).into_owned(),
    }
}

fn parse_notice(value: &Value) -> Option<Notice> {
    Some(Notice {
        seq: value.get("seq")?.as_u64()?,
        on_thread: value.get("kind").and_then(Value::as_str) == Some("thread_reply"),
        thread_id: value.get("thread_id")?.as_u64()?,
        from: str_field(value, "from"),
        body: str_field(value, "body"),
        created_at: value.get("created_at").and_then(Value::as_i64).unwrap_or(0),
    })
}

fn str_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn parse_summary(value: &Value) -> Option<ThreadSummary> {
    Some(ThreadSummary {
        id: value.get("id")?.as_u64()?,
        author: str_field(value, "author"),
        title: str_field(value, "title"),
        body: str_field(value, "body"),
        created_at: value.get("created_at").and_then(Value::as_i64).unwrap_or(0),
        bumped: value.get("bumped").and_then(Value::as_u64).unwrap_or(0),
        comments: value.get("comments").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn parse_threads(value: &Value) -> Vec<ThreadSummary> {
    value.as_array().map(|list| list.iter().filter_map(parse_summary).collect()).unwrap_or_default()
}

/// A thread answer: the thread's fields, with `comments` the list of comments
/// instead of their count.
fn parse_thread(value: &Value) -> Option<Thread> {
    let list = value.get("comments")?.as_array()?;
    let comments: Vec<Comment> = list
        .iter()
        .filter_map(|c| {
            Some(Comment {
                id: c.get("id")?.as_u64()?,
                author: str_field(c, "author"),
                body: str_field(c, "body"),
                created_at: c.get("created_at").and_then(Value::as_i64).unwrap_or(0),
                depth: c.get("depth").and_then(Value::as_u64).map_or(0, |d| d.min(u64::from(u32::MAX)) as u32),
            })
        })
        .collect();
    let mut summary = parse_summary(value)?;
    summary.comments = comments.len() as u64;
    Some(Thread { summary, comments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;
    use serde_json::json;

    /// Every character is 6 pixels wide.
    fn mono(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    fn build(win: &mut ForumWindow, ui_ctx: &mut UiContext, state: &mut StateCache) -> Vec<GameEvent> {
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ui = test_frame(ui_ctx, state);
        win.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
    }

    fn requests(events: &[GameEvent]) -> Vec<ForumRequest> {
        events
            .iter()
            .filter_map(|e| match e {
                GameEvent::RequestForum(request) => Some(request.clone()),
                _ => None,
            })
            .collect()
    }

    fn thread_json(id: u64, bumped: u64) -> Value {
        json!({ "id": id, "author": "Alice", "title": format!("Thread {id}"), "body": "Body",
                "created_at": 1000, "bumped": bumped, "comments": 2 })
    }

    #[test]
    fn lines_break_after_spaces_inside_long_words_and_at_line_breaks() {
        let lines = wrap_lines(&mono, "aaa bbb ccc", 40.0);
        assert_eq!(lines, vec![(0, "aaa ".to_string()), (4, "bbb ".to_string()), (8, "ccc".to_string())]);
        let lines = wrap_lines(&mono, "abcdefghij", 24.0);
        assert_eq!(lines, vec![(0, "abcd".into()), (4, "efgh".into()), (8, "ij".into())]);
        let lines = wrap_lines(&mono, "one\n\ntwo\n", 600.0);
        assert_eq!(lines, vec![(0, "one".into()), (4, String::new()), (5, "two".into()), (9, String::new())]);
        assert_eq!(wrap_lines(&mono, "", 60.0), vec![(0, String::new())]);
    }

    #[test]
    fn the_cursor_is_placed_on_its_wrapped_line_and_back() {
        let lines = wrap_lines(&mono, "aaa bbb\nccc", 40.0);
        assert_eq!(cursor_place(&mono, &lines, 0), (0, 0.0));
        assert_eq!(cursor_place(&mono, &lines, 5), (1, 6.0));
        assert_eq!(cursor_place(&mono, &lines, 8), (2, 0.0));
        assert_eq!(cursor_place(&mono, &lines, 11), (2, 18.0));
        assert_eq!(index_at(&mono, &lines, 1, 7.0), 5);
        assert_eq!(index_at(&mono, &lines, 0, 999.0), 3, "stays before the space the line was broken at");
        assert_eq!(index_at(&mono, &lines, 2, 999.0), 11);
    }

    #[test]
    fn ages_read_in_the_largest_whole_unit() {
        assert_eq!(age(1000, 990), "just now");
        assert_eq!(age(1000, 1100), "just now", "a clock behind the server is not negative");
        assert_eq!(age(10_000, 10_000 - 125), "2m ago");
        assert_eq!(age(100_000, 100_000 - 7200), "2h ago");
        assert_eq!(age(1_000_000, 1_000_000 - 3 * 86400), "3d ago");
    }

    #[test]
    fn opening_asks_for_the_threads_and_a_full_page_offers_more() {
        let mut win = ForumWindow::new();
        win.toggle();
        let mut state = StateCache::new();
        let events = build(&mut win, &mut UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&events), vec![ForumRequest::Threads { before: None, limit: PAGE_SIZE }]);

        let page: Vec<Value> = (0..PAGE_SIZE as u64).map(|n| thread_json(100 - n, 500 - n)).collect();
        win.deliver(&ForumRequest::Threads { before: None, limit: PAGE_SIZE }, Ok(Value::Array(page)));
        assert!(win.has_more);
        win.deliver(
            &ForumRequest::Threads { before: Some(476), limit: PAGE_SIZE },
            Ok(json!([thread_json(75, 475), thread_json(76, 476)])),
        );
        assert!(!win.has_more, "a short page is the last one");
        let Some(Load::Ready(list)) = &win.threads else { panic!("no list") };
        assert_eq!(list.len(), PAGE_SIZE + 1, "a thread already listed is not listed twice");
    }

    #[test]
    fn clicking_a_thread_asks_for_it_with_its_comments() {
        let mut win = ForumWindow::new();
        win.toggle();
        let mut state = StateCache::new();
        build(&mut win, &mut UiContext::new(800.0, 600.0), &mut state);
        win.deliver(&ForumRequest::Threads { before: None, limit: PAGE_SIZE }, Ok(json!([thread_json(7, 1)])));

        let mut ui_ctx = UiContext::new(800.0, 600.0);
        ui_ctx.mouse_x = 160.0 + PAD + 40.0;
        ui_ctx.mouse_y = 110.0 + TITLE_H + TOOL_H + 10.0;
        ui_ctx.mouse_clicked = true;
        build(&mut win, &mut ui_ctx, &mut state);
        assert_eq!(win.view, View::Thread(7));
        let events = build(&mut win, &mut UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&events), vec![ForumRequest::Thread { id: 7 }]);
    }

    fn open_on_thread(win: &mut ForumWindow, state: &mut StateCache) {
        win.toggle();
        win.open_thread(7);
        build(win, &mut UiContext::new(800.0, 600.0), state);
        let thread = json!({ "id": 7, "author": "Alice", "title": "Hi", "body": "First post", "created_at": 1,
            "bumped": 3, "comments": [
                { "id": 11, "thread_id": 7, "parent": null, "author": "Bob", "body": "A comment", "created_at": 2, "depth": 0 },
                { "id": 12, "thread_id": 7, "parent": 11, "author": "Alice", "body": "A reply", "created_at": 3, "depth": 1 }
            ] });
        win.deliver(&ForumRequest::Thread { id: 7 }, Ok(thread));
    }

    #[test]
    fn a_thread_lays_out_its_comments_under_what_they_answer() {
        let Some(thread) = parse_thread(&json!({ "id": 7, "author": "Alice", "title": "Hi", "body": "First post",
            "created_at": 1, "bumped": 3, "comments": [
                { "id": 11, "author": "Bob", "body": "A comment", "created_at": 2, "depth": 0 },
                { "id": 12, "author": "Alice", "body": "A reply", "created_at": 3, "depth": 1 }
            ] })) else { panic!("unparsed") };
        let rows = thread_rows(&mono, &thread, 400.0);
        let shown: Vec<(RowKind, u8, &str)> = rows.iter().map(|r| (r.kind, r.indent, r.text.as_str())).collect();
        assert_eq!(
            shown,
            vec![
                (RowKind::Title, 0, "Hi"),
                (RowKind::Meta, 0, "Alice"),
                (RowKind::Body, 0, "First post"),
                (RowKind::Gap, 0, ""),
                (RowKind::Meta, 0, "Bob"),
                (RowKind::Body, 0, "A comment"),
                (RowKind::Gap, 0, ""),
                (RowKind::Meta, 1, "Alice"),
                (RowKind::Body, 1, "A reply"),
                (RowKind::Gap, 1, ""),
            ]
        );
        assert_eq!(rows[4].reply, Some((11, "Bob".to_string())));
        assert_eq!(rows[1].extra.as_deref(), Some("2 comments"));
    }

    #[test]
    fn enter_in_the_comment_box_sends_a_reply_and_success_clears_it() {
        let mut win = ForumWindow::new();
        let mut state = StateCache::new();
        open_on_thread(&mut win, &mut state);
        win.reply_to = Some((11, "Bob".to_string()));
        win.comment.input.text = "Thanks!".to_string();
        win.comment.input.cursor_pos = 7;

        let mut ui_ctx = UiContext::new(800.0, 600.0);
        {
            let mut ui = test_frame(&mut ui_ctx, &mut state);
            ui.set_focus(COMMENT_INPUT_ID);
        }
        ui_ctx.key_enter = true;
        let events = build(&mut win, &mut ui_ctx, &mut state);
        let sent = ForumRequest::Comment { thread_id: 7, body: "Thanks!".to_string(), parent: Some(11) };
        assert_eq!(requests(&events), vec![sent.clone()]);
        assert!(win.is_typing());

        win.deliver(&sent, Ok(json!({ "id": 13 })));
        assert_eq!(win.comment.text(), "");
        assert_eq!(win.reply_to, None);
        let events = build(&mut win, &mut UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&events), vec![ForumRequest::Thread { id: 7 }], "the thread is read again");
    }

    #[test]
    fn shift_enter_starts_a_new_line_and_a_refused_comment_keeps_its_text() {
        let mut win = ForumWindow::new();
        let mut state = StateCache::new();
        open_on_thread(&mut win, &mut state);
        win.comment.input.text = "one".to_string();
        win.comment.input.cursor_pos = 3;
        let mut ui_ctx = UiContext::new(800.0, 600.0);
        {
            let mut ui = test_frame(&mut ui_ctx, &mut state);
            ui.set_focus(COMMENT_INPUT_ID);
        }
        ui_ctx.key_enter = true;
        ui_ctx.shift_pressed = true;
        let events = build(&mut win, &mut ui_ctx, &mut state);
        assert!(requests(&events).is_empty());
        assert_eq!(win.comment.text(), "one\n");

        let sent = ForumRequest::Comment { thread_id: 7, body: "one".to_string(), parent: None };
        win.posting = Some(sent.clone());
        win.deliver(&sent, Err("wait 30 more seconds".to_string()));
        assert_eq!(win.comment.text(), "one\n");
        assert_eq!(win.status, Some(Status::Error("wait 30 more seconds".to_string())));
        assert_eq!(win.posting, None);
    }

    #[test]
    fn a_posted_thread_is_opened() {
        let mut win = ForumWindow::new();
        win.toggle();
        win.start_thread();
        win.title.text = "Hello".to_string();
        win.body.input.text = "Text".to_string();
        let request = ForumRequest::CreateThread { title: "Hello".to_string(), body: "Text".to_string() };
        win.deliver(&request, Ok(thread_json(42, 9)));
        assert_eq!(win.view, View::Thread(42));
        assert_eq!(win.title.text, "");
        assert_eq!(win.body.text(), "");
        assert!(win.threads.is_none(), "the list is read again");
    }

    fn notice_json(seq: u64, kind: &str, thread_id: u64) -> Value {
        json!({ "event": "reply", "for": "Hero", "seq": seq, "kind": kind, "thread_id": thread_id,
                "comment_id": seq, "parent": null, "from": "Bob",
                "body": "Does the offer\nstill stand?", "created_at": 1000 })
    }

    #[test]
    fn a_few_replies_get_a_chat_line_each_and_a_backlog_one_line() {
        let mut win = ForumWindow::new();
        let lines = win.add_notices(&[notice_json(1, "thread_reply", 4), notice_json(2, "comment_reply", 4)]);
        assert_eq!(
            lines,
            vec![
                "[Forum] Bob replied to your thread: Does the offer still stand?".to_string(),
                "[Forum] Bob replied to your comment: Does the offer still stand?".to_string(),
            ]
        );
        assert_eq!(win.unread, 2);
        assert!(win.add_notices(&[notice_json(2, "comment_reply", 4)]).is_empty(), "a reply already had is ignored");

        let backlog: Vec<Value> = (10..15).map(|seq| notice_json(seq, "thread_reply", 4)).collect();
        let lines = win.add_notices(&backlog);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("5 new replies"));
        assert_eq!(win.unread, 7);
        assert_eq!(win.notices.first().map(|n| n.seq), Some(14), "the newest comes first");
    }

    #[test]
    fn a_reply_on_the_thread_on_show_reads_it_again() {
        let mut win = ForumWindow::new();
        let mut state = StateCache::new();
        open_on_thread(&mut win, &mut state);
        win.add_notices(&[notice_json(1, "thread_reply", 7)]);
        let events = build(&mut win, &mut UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&events), vec![ForumRequest::Thread { id: 7 }]);
    }

    #[test]
    fn a_reply_opens_its_thread_and_back_returns_to_the_replies() {
        let mut win = ForumWindow::new();
        win.toggle();
        win.add_notices(&[notice_json(1, "thread_reply", 9)]);
        win.open_replies();
        assert_eq!(win.unread, 0);
        let mut state = StateCache::new();
        let mut ui_ctx = UiContext::new(800.0, 600.0);
        ui_ctx.mouse_x = 160.0 + PAD + 40.0;
        ui_ctx.mouse_y = 110.0 + TITLE_H + TOOL_H + 10.0;
        ui_ctx.mouse_clicked = true;
        build(&mut win, &mut ui_ctx, &mut state);
        assert_eq!(win.view, View::Thread(9));
        win.go_back();
        assert_eq!(win.view, View::Replies);
        win.go_back();
        assert_eq!(win.view, View::List);
    }

    #[test]
    fn empty_and_long_texts_are_refused_before_sending() {
        assert!(check_text("", 10).is_some());
        assert!(check_text("12345678901", 10).is_some());
        assert_eq!(check_text("ok", 10), None);
    }
}
