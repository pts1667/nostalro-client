//! In-game browser for the server's read-only `/db` reference (items, monsters,
//! skills, maps, ...).
//!
//! The window does no networking. It asks for a page with
//! [`GameEvent::RequestReference`] and the client hands the answer back through
//! [`ReferenceWindow::deliver`], so the same code runs in the gallery and the
//! game. Entries differ per category, so a page is rendered generically from its
//! JSON: scalars become `label: value` rows and every `{ id, name, href }` link
//! the server emits becomes a clickable row that opens the linked entry.

use std::collections::HashMap;

use crate::game::context_menu::info_menu;
use crate::helper::scrollbar::{SCROLLBAR_W, ScrollbarIds, scrollbar};
use crate::helper::window_chrome::{
    draw_container, draw_footer, draw_sys_button, draw_titlebar, label_color, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::data_table::DataTable;
use ragnarok_game::event::GameEvent;
use ragnarok_game::skill::skill_icon_path_for_name;
use ragnarok_game::sprite_path::{entity_sprite_base_path, sprite_thumbnail_key};
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;
use ragnarok_ui::theme::FallbackPalette as P;
use serde_json::Value;

pub const REFERENCE_WINDOW_ID: WidgetId = WidgetId(6000);
const CLOSE_BTN_ID: WidgetId = WidgetId(6001);
const BACK_BTN_ID: WidgetId = WidgetId(6002);
const FORWARD_BTN_ID: WidgetId = WidgetId(6003);
const HOME_BTN_ID: WidgetId = WidgetId(6004);
const SEARCH_INPUT_ID: WidgetId = WidgetId(6005);
const SEARCH_BTN_ID: WidgetId = WidgetId(6006);
const PREV_PAGE_BTN_ID: WidgetId = WidgetId(6007);
const NEXT_PAGE_BTN_ID: WidgetId = WidgetId(6008);
const RETRY_BTN_ID: WidgetId = WidgetId(6009);
const SCROLL_UP_ID: WidgetId = WidgetId(6010);
const SCROLL_DOWN_ID: WidgetId = WidgetId(6011);
const SCROLL_THUMB_ID: WidgetId = WidgetId(6012);
const LINK_ROW_BASE_ID: u32 = 6020;

const WIN_W: f32 = 480.0;
const TITLE_H: f32 = 17.0;
const TOOL_H: f32 = 26.0;
const ROW_H: f32 = 15.0;
const VISIBLE_ROWS: usize = 15;
const LIST_H: f32 = ROW_H * VISIBLE_ROWS as f32;
/// Space under the list so its border shows above the footer.
const WELL_GAP: f32 = 5.0;
const FOOTER_H: f32 = 26.0;
const WIN_H: f32 = TITLE_H + TOOL_H + LIST_H + WELL_GAP + FOOTER_H;
const PAD: f32 = 6.0;
const BASELINE: f32 = 12.0;
const INDENT_W: f32 = 12.0;
const SYS_BTN: f32 = 11.0;
const NAV_BTN_W: f32 = 22.0;
const BTN_H: f32 = 20.0;
const TEXT_BTN_W: f32 = 42.0;

/// A row holding a picture is this many text rows tall.
const ICON_ROW_UNITS: usize = 2;
/// The banner opening an entry.
const BANNER_UNITS: usize = 3;
const ICON_BOX: f32 = 24.0;
const BANNER_BOX: f32 = 40.0;
/// Largest enlargement of a small picture in the banner.
const BANNER_MAX_SCALE: f32 = 1.6;

/// Results asked of a category per page; the server allows up to 200.
const PAGE_SIZE: usize = 100;
/// Back/forward history kept, oldest dropped first.
const HISTORY_MAX: usize = 100;
/// A list of plain values is shown on one row up to this many characters.
const JOIN_MAX_CHARS: usize = 70;

const ROOT_PATH: &str = "/db";
const LINK_COLOR: [f32; 4] = [0.10, 0.25, 0.75, 1.0];
const HOVER_ROW: [f32; 4] = [0.78, 0.86, 1.0, 0.7];
const MUTED_COLOR: [f32; 4] = [0.45, 0.47, 0.55, 1.0];
const ERROR_COLOR: [f32; 4] = [0.75, 0.12, 0.12, 1.0];

/// What a picture stands for. It is turned into a texture path when drawn,
/// because that needs the client's data tables.
#[derive(Clone, Debug, PartialEq, Eq)]
enum IconRef {
    Item(u16),
    /// The skill's name as the server spells it (`NV_BASIC`).
    Skill(String),
    /// A monster, by job id; shown as its sprite.
    Mob(u16),
}

/// One row of a page.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    indent: u8,
    label: Option<String>,
    text: String,
    link: Option<String>,
    heading: bool,
    icon: Option<IconRef>,
    /// The large picture and name that open an entry.
    banner: bool,
    /// Never shares a row: a wrapped piece of text.
    no_pack: bool,
}

impl Line {
    fn value(indent: u8, label: Option<String>, text: String) -> Self {
        Self { indent, label, text, link: None, heading: false, icon: None, banner: false, no_pack: false }
    }

    fn link(indent: u8, label: Option<String>, text: String, href: String) -> Self {
        Self { link: Some(href), ..Self::value(indent, label, text) }
    }

    fn heading(indent: u8, text: String) -> Self {
        Self { heading: true, ..Self::value(indent, None, text) }
    }

    fn banner(icon: IconRef, text: String) -> Self {
        Self { icon: Some(icon), banner: true, ..Self::value(0, None, text) }
    }

    fn with_icon(mut self, icon: Option<IconRef>) -> Self {
        self.icon = icon;
        self
    }

    /// Height in text rows.
    fn units(&self) -> usize {
        if self.banner {
            BANNER_UNITS
        } else if self.icon.is_some() {
            ICON_ROW_UNITS
        } else {
            1
        }
    }

    fn height(&self) -> f32 {
        self.units() as f32 * ROW_H
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Paging {
    total: usize,
    offset: usize,
    limit: usize,
}

struct Page {
    lines: Vec<Line>,
    paging: Option<Paging>,
    /// `lines` wrapped and set side by side; built on first draw, when the
    /// font is at hand to measure with.
    rows: Option<Vec<Row>>,
}

impl Page {
    fn new(lines: Vec<Line>, paging: Option<Paging>) -> Self {
        Self { lines, paging, rows: None }
    }
}

/// One or two lines shown side by side.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    cells: Vec<Line>,
}

impl Row {
    fn height(&self) -> f32 {
        self.cells.iter().map(Line::height).fold(ROW_H, f32::max)
    }
}

/// Where a picture the window asked the client for stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Texture {
    Requested,
    Ready(u32, u32),
    Failed,
}

enum Load {
    Pending,
    Ready(Page),
    Failed(String),
}

struct HistoryEntry {
    path: String,
    scroll: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Categories,
    List,
    Entry,
}

pub struct ReferenceWindow {
    open: bool,
    history: Vec<HistoryEntry>,
    cursor: usize,
    pages: HashMap<String, Load>,
    scroll: usize,
    search: TextInput,
    search_focused: bool,
    textures: HashMap<String, Texture>,
}

impl Default for ReferenceWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            history: vec![HistoryEntry { path: ROOT_PATH.to_string(), scroll: 0 }],
            cursor: 0,
            pages: HashMap::new(),
            scroll: 0,
            search: TextInput::new(40, false),
            search_focused: false,
            textures: HashMap::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Shows the window on `path`, for instance `/db/items/501`.
    pub fn open_at(&mut self, path: &str) {
        self.open = true;
        self.navigate(path.to_string());
    }

    /// Hands over the answer to a [`GameEvent::RequestReference`].
    pub fn deliver(&mut self, path: &str, result: Result<Value, String>) {
        let load = match result {
            Ok(value) => Load::Ready(build_page(path, &value)),
            Err(message) => Load::Failed(message),
        };
        self.pages.insert(path.to_string(), load);
    }

    /// Answers a [`GameEvent::RequestReferenceTexture`]: the size of the loaded
    /// picture, or `None` when it could not be had.
    pub fn texture_loaded(&mut self, path: &str, size: Option<(u32, u32)>) {
        let state = match size {
            Some((w, h)) if w > 0 && h > 0 => Texture::Ready(w, h),
            _ => Texture::Failed,
        };
        self.textures.insert(path.to_string(), state);
    }

    fn current_path(&self) -> &str {
        &self.history[self.cursor].path
    }

    fn navigate(&mut self, path: String) {
        if self.current_path() == path {
            return;
        }
        self.history[self.cursor].scroll = self.scroll;
        self.history.truncate(self.cursor + 1);
        self.history.push(HistoryEntry { path, scroll: 0 });
        if self.history.len() > HISTORY_MAX {
            self.history.remove(0);
        }
        self.cursor = self.history.len() - 1;
        self.scroll = 0;
        self.sync_search();
    }

    fn go_back(&mut self) {
        if self.cursor > 0 {
            self.step_to(self.cursor - 1);
        }
    }

    fn go_forward(&mut self) {
        if self.cursor + 1 < self.history.len() {
            self.step_to(self.cursor + 1);
        }
    }

    fn step_to(&mut self, index: usize) {
        self.history[self.cursor].scroll = self.scroll;
        self.cursor = index;
        self.scroll = self.history[index].scroll;
        self.sync_search();
    }

    /// The search box mirrors the `q` of the list on show and is empty elsewhere.
    fn sync_search(&mut self) {
        let text = match shape(self.current_path()) {
            Shape::List => query_of(self.current_path()),
            Shape::Categories | Shape::Entry => String::new(),
        };
        self.search.cursor_pos = text.chars().count();
        self.search.text = text;
        self.search.clear_selection();
    }

    fn run_search(&mut self) {
        let Some(category) = category_of(self.current_path()).map(str::to_string) else {
            return;
        };
        let path = list_path(&category, &self.search.text, 0);
        self.navigate(path);
    }

    fn draw_toolbar(&mut self, ui: &mut UiFrame, x: f32, y: f32, target: &mut Option<String>) {
        let by = y + (TOOL_H - BTN_H) / 2.0;
        let mut bx = x + PAD;

        let back = Rect::new(bx, by, NAV_BTN_W, BTN_H);
        if ui.button(BACK_BTN_ID, back, "<").clicked() {
            self.go_back();
        }
        bx += NAV_BTN_W + 2.0;
        let forward = Rect::new(bx, by, NAV_BTN_W, BTN_H);
        if ui.button(FORWARD_BTN_ID, forward, ">").clicked() {
            self.go_forward();
        }
        bx += NAV_BTN_W + 2.0;
        let home = Rect::new(bx, by, TEXT_BTN_W, BTN_H);
        if ui.button(HOME_BTN_ID, home, &rust_i18n::t!("reference.home")).clicked() {
            *target = Some(ROOT_PATH.to_string());
        }
        bx += TEXT_BTN_W + PAD;

        let right = x + WIN_W - PAD;
        if category_of(self.current_path()).is_some() {
            let go = Rect::new(right - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
            let input = Rect::new(bx, by + 2.0, go.x - bx - 3.0, BTN_H - 4.0);
            let resp = ui.text_input(SEARCH_INPUT_ID, input, &mut self.search, TextInputBg::Default);
            self.search_focused = resp.has_focus();
            let submitted = self.search_focused && ui.enter_pressed();
            if ui.button(SEARCH_BTN_ID, go, &rust_i18n::t!("reference.search")).clicked() || submitted {
                self.run_search();
            }
        } else {
            self.search_focused = false;
            ui.text(bx, by + BASELINE + 2.0, &rust_i18n::t!("reference.pick_category"), label_color());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_rows(
        &mut self,
        ui: &mut UiFrame,
        rows: &[Row],
        well: Rect,
        data: &DataTable,
        events: &mut Vec<GameEvent>,
        target: &mut Option<String>,
    ) {
        let heights: Vec<f32> = rows.iter().map(Row::height).collect();
        let max_scroll = max_scroll(&heights);
        let bar_x = well.x + well.w - SCROLLBAR_W;
        let content = Rect::new(well.x, well.y, well.w - SCROLLBAR_W, well.h);
        self.scroll = scrollbar(
            ui,
            ScrollbarIds { up: SCROLL_UP_ID, down: SCROLL_DOWN_ID, thumb: SCROLL_THUMB_ID },
            self.scroll,
            visible_count(&heights, self.scroll.min(max_scroll)),
            max_scroll,
            content,
            bar_x,
            well.y,
            well.h,
        );

        let full_w = content.w - 2.0;
        let mut top = well.y;
        let mut slot = 0u32;
        for (row, &height) in rows.iter().zip(&heights).skip(self.scroll) {
            if top + height > well.y + well.h + 0.5 {
                break;
            }
            let cell_w = if row.cells.len() > 1 { (full_w / 2.0).floor() } else { full_w };
            for (col, cell) in row.cells.iter().enumerate() {
                let rect = Rect::new(content.x + 1.0 + col as f32 * cell_w, top, cell_w, height);
                self.draw_cell(ui, cell, rect, &mut slot, data, events, target);
            }
            top += height;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_cell(
        &mut self,
        ui: &mut UiFrame,
        line: &Line,
        row: Rect,
        slot: &mut u32,
        data: &DataTable,
        events: &mut Vec<GameEvent>,
        target: &mut Option<String>,
    ) {
        let hovered = match &line.link {
            Some(href) => {
                let resp = ui.interact(WidgetId(LINK_ROW_BASE_ID + *slot), row);
                *slot += 1;
                if resp.hovered() {
                    ui.any_interactive_hovered = true;
                    quad(ui, row.x, row.y, row.w - 1.0, row.h, HOVER_ROW);
                }
                if resp.clicked() {
                    *target = Some(href.clone());
                }
                if resp.right_clicked() {
                    events.push(info_menu(ui, Some(href.clone()), None));
                }
                resp.hovered()
            }
            None => false,
        };

        let right = row.x + row.w - 3.0;
        let baseline = row.y + (row.h / 2.0 + 4.5).floor();
        let mut x = row.x + 3.0 + f32::from(line.indent) * INDENT_W;
        if let Some(icon) = &line.icon {
            let (side, max_scale, gap) =
                if line.banner { (BANNER_BOX, BANNER_MAX_SCALE, 8.0) } else { (ICON_BOX, 1.0, 6.0) };
            let boxed = Rect::new(x, (row.y + (row.h - side) / 2.0).floor(), side, side);
            self.draw_icon(ui, icon, boxed, max_scale, data, events);
            x += side + gap;
        }
        if let Some(label) = &line.label {
            let shown = format!("{label}:");
            clipped_text(ui, x, baseline, &shown, label_color(), right);
            x += ui.atlas.measure_text(&shown) + 5.0;
        }
        if line.heading || line.banner {
            for dx in [0.0, 1.0] {
                let (vertices, indices) =
                    draw::text_vertices(&line.text, x + dx, baseline, label_color(), ui.atlas);
                if !vertices.is_empty() {
                    ui.draw_calls.push(DrawCall { vertices, indices, texture: TextureRef::FontAtlas });
                }
            }
        } else if line.link.is_some() {
            clipped_text(ui, x, baseline, &line.text, LINK_COLOR, right);
            if hovered {
                let w = ui.atlas.measure_text(&line.text).min(right - x);
                quad(ui, x, baseline + 2.0, w, 1.0, LINK_COLOR);
            }
        } else {
            clipped_text(ui, x, baseline, &line.text, text_color(), right);
        }
    }

    /// Draws `icon` fitted in `boxed`, asking the client for the picture the
    /// first time it is on show.
    fn draw_icon(
        &mut self,
        ui: &mut UiFrame,
        icon: &IconRef,
        boxed: Rect,
        max_scale: f32,
        data: &DataTable,
        events: &mut Vec<GameEvent>,
    ) {
        let Some(path) = icon_path(icon, data) else {
            return;
        };
        match self.textures.get(&path) {
            None => {
                self.textures.insert(path.clone(), Texture::Requested);
                events.push(GameEvent::RequestReferenceTexture { path });
            }
            Some(&Texture::Ready(w, h)) => {
                let (w, h) = (w as f32, h as f32);
                let scale = (boxed.w / w).min(boxed.h / h).min(max_scale);
                let (dw, dh) = ((w * scale).round(), (h * scale).round());
                let x = (boxed.x + (boxed.w - dw) / 2.0).floor();
                let y = (boxed.y + (boxed.h - dh) / 2.0).floor();
                let (v, i) = draw::quad_vertices(x, y, dw, dh, [1.0; 4]);
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: i.to_vec(),
                    texture: TextureRef::Named(path),
                });
            }
            Some(&Texture::Requested) | Some(&Texture::Failed) => {}
        }
    }

    fn draw_footer_row(
        &mut self,
        ui: &mut UiFrame,
        x: f32,
        y: f32,
        paging: Option<Paging>,
        target: &mut Option<String>,
    ) {
        let by = y + (FOOTER_H - BTN_H) / 2.0;
        let right = x + WIN_W - PAD;
        let Some(paging) = paging else {
            let path = self.current_path().to_string();
            clipped_text(ui, x + PAD, y + FOOTER_H / 2.0 + 4.0, &path, MUTED_COLOR, right);
            return;
        };

        let from = if paging.total == 0 { 0 } else { paging.offset + 1 };
        let to = (paging.offset + paging.limit).min(paging.total);
        let range = rust_i18n::t!("reference.range", from = from, to = to, total = paging.total);
        ui.text(x + PAD, y + FOOTER_H / 2.0 + 4.0, &range, text_color());

        let next = Rect::new(right - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
        let prev = Rect::new(next.x - 3.0 - TEXT_BTN_W, by, TEXT_BTN_W, BTN_H);
        let step = paging.limit.max(1);
        if paging.offset > 0
            && ui.button(PREV_PAGE_BTN_ID, prev, &rust_i18n::t!("reference.prev")).clicked()
        {
            *target = Some(with_offset(self.current_path(), paging.offset.saturating_sub(step)));
        }
        if paging.offset + step < paging.total
            && ui.button(NEXT_PAGE_BTN_ID, next, &rust_i18n::t!("reference.next")).clicked()
        {
            *target = Some(with_offset(self.current_path(), paging.offset + step));
        }
    }
}

impl Window for ReferenceWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for ReferenceWindow {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.open && self.search_focused
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            self.search_focused = false;
            return Vec::new();
        }
        let mut events = Vec::new();
        let tc = text_color();

        let win = ui.window_at(REFERENCE_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 120.0, 90.0);
        let (x, y) = (win.x, win.y);
        ui.interact(REFERENCE_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + 13.0, &rust_i18n::t!("reference.title"), tc);
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
        draw_container(ui, x, body_y, WIN_W, TOOL_H + LIST_H + WELL_GAP);

        let path = self.current_path().to_string();
        if !self.pages.contains_key(&path) {
            self.pages.insert(path.clone(), Load::Pending);
            events.push(GameEvent::RequestReference { path: path.clone() });
        }

        let mut target: Option<String> = None;
        self.draw_toolbar(ui, x, body_y, &mut target);

        let well = Rect::new(x + PAD, body_y + TOOL_H, WIN_W - PAD * 2.0, LIST_H);
        quad(ui, well.x - 1.0, well.y - 1.0, well.w + 2.0, well.h + 2.0, P::WIN_BORDER);
        quad(ui, well.x, well.y, well.w, well.h, P::SLOT_FILL);

        // Taken out of the cache so the lines can be drawn while `self` is borrowed mutably.
        let mut load = self.pages.remove(&path);
        let mut paging = None;
        let mut retry = false;
        match &mut load {
            Some(Load::Ready(page)) => {
                paging = page.paging;
                if page.lines.is_empty() {
                    ui.text(well.x + 6.0, well.y + BASELINE + 2.0, &rust_i18n::t!("reference.no_results"), MUTED_COLOR);
                } else {
                    let full_w = well.w - SCROLLBAR_W - 2.0;
                    let rows = page.rows.get_or_insert_with(|| {
                        layout(&|t: &str| ui.atlas.measure_text(t), &page.lines, full_w)
                    });
                    self.draw_rows(ui, rows, well, ctx.data, &mut events, &mut target);
                }
            }
            Some(Load::Failed(message)) => {
                let shown = rust_i18n::t!("reference.error", message = message.as_str());
                clipped_text(ui, well.x + 6.0, well.y + BASELINE + 2.0, &shown, ERROR_COLOR, well.x + well.w - 4.0);
                let retry_rect = Rect::new(well.x + 6.0, well.y + 26.0, TEXT_BTN_W + 10.0, BTN_H);
                // Leaving the entry out of the cache makes the next frame ask again.
                retry = ui.button(RETRY_BTN_ID, retry_rect, &rust_i18n::t!("reference.retry")).clicked();
            }
            Some(Load::Pending) | None => {
                ui.text(well.x + 6.0, well.y + BASELINE + 2.0, &rust_i18n::t!("reference.loading"), MUTED_COLOR);
            }
        }
        if let Some(load) = load
            && !retry
        {
            self.pages.insert(path, load);
        }

        let footer_y = y + TITLE_H + TOOL_H + LIST_H + WELL_GAP;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        self.draw_footer_row(ui, x, footer_y, paging, &mut target);

        if let Some(next) = target {
            self.navigate(next);
        }
        events
    }
}

fn quad(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (v, i) = draw::quad_vertices(x, y, w, h, color);
    ui.draw_calls.push(DrawCall { vertices: v.to_vec(), indices: i.to_vec(), texture: TextureRef::White });
}

/// Text cut off at `right` instead of spilling out of the list.
fn clipped_text(ui: &mut UiFrame, x: f32, y: f32, text: &str, color: [f32; 4], right: f32) {
    let (vertices, indices) =
        draw::text_vertices_clipped(text, x, y, color, ui.atlas, f32::NEG_INFINITY, right);
    if !vertices.is_empty() {
        ui.draw_calls.push(DrawCall { vertices, indices, texture: TextureRef::FontAtlas });
    }
}

/// Pixels a cell needs: indent, picture, label and text.
fn cell_width(measure: &dyn Fn(&str) -> f32, line: &Line) -> f32 {
    let mut w = 3.0 + f32::from(line.indent) * INDENT_W;
    if line.icon.is_some() {
        w += if line.banner { BANNER_BOX + 8.0 } else { ICON_BOX + 6.0 };
    }
    if let Some(label) = &line.label {
        w += measure(&format!("{label}:")) + 5.0;
    }
    w + measure(&line.text)
}

/// `text` cut into pieces no wider than `max_w`, at spaces where it can and
/// inside a word where it has to; newlines start a new piece.
fn wrap_text(measure: &dyn Fn(&str) -> f32, text: &str, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word.to_string();
            loop {
                let candidate = if line.is_empty() { word.clone() } else { format!("{line} {word}") };
                if measure(&candidate) <= max_w {
                    line = candidate;
                    break;
                }
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    continue;
                }
                // A single word wider than the line: cut it.
                let chars: Vec<char> = word.chars().collect();
                let mut n = 1;
                while n < chars.len() && measure(&chars[..=n].iter().collect::<String>()) <= max_w {
                    n += 1;
                }
                out.push(chars[..n].iter().collect());
                word = chars[n..].iter().collect();
                if word.is_empty() {
                    break;
                }
            }
        }
        if !line.is_empty() || paragraph.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn packable(line: &Line) -> bool {
    !line.heading && !line.banner && !line.no_pack
}

/// Wraps long plain text, then sets neighbouring short lines of the same kind
/// side by side so the width of the list is used.
fn layout(measure: &dyn Fn(&str) -> f32, lines: &[Line], full_w: f32) -> Vec<Row> {
    let mut flat: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines {
        let plain = line.link.is_none() && line.icon.is_none() && packable(line);
        if plain && (line.text.contains('\n') || cell_width(measure, line) > full_w - 8.0) {
            let head = cell_width(measure, &Line { text: String::new(), ..line.clone() });
            let pieces = wrap_text(measure, &line.text, (full_w - 8.0 - head).max(60.0));
            for (n, piece) in pieces.into_iter().enumerate() {
                flat.push(if n == 0 {
                    Line { text: piece, no_pack: true, ..line.clone() }
                } else {
                    Line { label: None, text: piece, indent: line.indent + 1, no_pack: true, ..line.clone() }
                });
            }
        } else {
            flat.push(line.clone());
        }
    }

    let column_limit = (full_w / 2.0).floor() - 8.0;
    let fits = |line: &Line| cell_width(measure, line) <= column_limit;
    let mut rows = Vec::with_capacity(flat.len());
    let mut i = 0;
    while i < flat.len() {
        let a = &flat[i];
        let pair = flat.get(i + 1).filter(|b| {
            packable(a)
                && packable(b)
                && a.indent == b.indent
                && a.icon.is_some() == b.icon.is_some()
                && a.link.is_some() == b.link.is_some()
                && fits(a)
                && fits(b)
        });
        match pair {
            Some(b) => {
                rows.push(Row { cells: vec![a.clone(), b.clone()] });
                i += 2;
            }
            None => {
                rows.push(Row { cells: vec![a.clone()] });
                i += 1;
            }
        }
    }
    rows
}

/// First line to scroll to so that the last line still ends inside the list.
fn max_scroll(heights: &[f32]) -> usize {
    let mut start = heights.len();
    let mut used = 0.0;
    while start > 0 && used + heights[start - 1] <= LIST_H {
        start -= 1;
        used += heights[start];
    }
    start
}

/// How many lines fit in the list from `start` on.
fn visible_count(heights: &[f32], start: usize) -> usize {
    let mut used = 0.0;
    let mut count = 0;
    for &h in heights.iter().skip(start) {
        if used + h > LIST_H {
            break;
        }
        used += h;
        count += 1;
    }
    count.max(1)
}

fn icon_path(icon: &IconRef, data: &DataTable) -> Option<String> {
    match icon {
        IconRef::Item(id) => data.item_resource.as_ref()?.item_icon_path(*id),
        IconRef::Skill(name) => Some(skill_icon_path_for_name(name)),
        IconRef::Mob(job) => {
            let base = entity_sprite_base_path(data.name.as_ref()?, *job)?;
            Some(sprite_thumbnail_key(&base))
        }
    }
}

// --- paths ---------------------------------------------------------------

fn shape(path: &str) -> Shape {
    let base = path.split('?').next().unwrap_or("");
    let rest = base.strip_prefix(ROOT_PATH).unwrap_or("");
    match rest.split('/').filter(|s| !s.is_empty()).count() {
        0 => Shape::Categories,
        1 => Shape::List,
        _ => Shape::Entry,
    }
}

/// The category a path points into, if it points into one.
fn category_of(path: &str) -> Option<&str> {
    let base = path.split('?').next()?;
    base.strip_prefix(ROOT_PATH)?.split('/').find(|s| !s.is_empty())
}

pub fn list_path(category: &str, query: &str, offset: usize) -> String {
    let mut path = format!("{ROOT_PATH}/{category}?limit={PAGE_SIZE}");
    let query = query.trim();
    if !query.is_empty() {
        path.push_str("&q=");
        path.push_str(&percent_encode(query));
    }
    if offset > 0 {
        path.push_str(&format!("&offset={offset}"));
    }
    path
}

fn query_of(path: &str) -> String {
    let Some((_, query)) = path.split_once('?') else {
        return String::new();
    };
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("q="))
        .map(percent_decode)
        .unwrap_or_default()
}

/// `path` with its `offset` parameter replaced (or dropped for 0).
fn with_offset(path: &str, offset: usize) -> String {
    let (base, query) = path.split_once('?').unwrap_or((path, ""));
    let mut pairs: Vec<String> = query
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("offset="))
        .map(str::to_string)
        .collect();
    if offset > 0 {
        pairs.push(format!("offset={offset}"));
    }
    if pairs.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", pairs.join("&"))
    }
}

/// The database path of an NPC: the map it stands on and its name.
pub fn npc_path(map: &str, name: &str) -> String {
    let map = map.split('.').next().unwrap_or(map);
    format!("{ROOT_PATH}/npcs/{}/{}", percent_encode(map), percent_encode(name))
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// --- JSON to rows ----------------------------------------------------------

fn build_page(path: &str, value: &Value) -> Page {
    match shape(path) {
        Shape::Categories => categories_page(value),
        Shape::List => list_page(value),
        Shape::Entry => {
            let mut lines = Vec::new();
            lines.extend(banner_of(path, value));
            push_value(&mut lines, 0, None, value);
            Page::new(lines, None)
        }
    }
}

fn categories_page(value: &Value) -> Page {
    let lines = value
        .get("categories")
        .and_then(Value::as_array)
        .map(|categories| {
            categories
                .iter()
                .filter_map(|c| {
                    let name = c.get("name")?.as_str()?;
                    let href = c.get("href")?.as_str()?;
                    let count = c.get("count").and_then(scalar_text).unwrap_or_default();
                    Some(Line::link(0, None, format!("{name}  ({count})"), href.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Page::new(lines, None)
}

fn list_page(value: &Value) -> Page {
    let number = |key: &str| value.get(key).and_then(Value::as_u64).map(|n| n as usize);
    let lines: Vec<Line> = value
        .get("results")
        .and_then(Value::as_array)
        .map(|results| {
            results
                .iter()
                .filter(|r| is_link(r))
                .map(|r| {
                    Line::link(0, None, link_text(r), link_href(r).unwrap_or_default().to_string())
                        .with_icon(icon_of(r))
                })
                .collect()
        })
        .unwrap_or_default();
    let paging = number("total").map(|total| Paging {
        total,
        offset: number("offset").unwrap_or(0),
        limit: number("limit").unwrap_or(lines.len()).max(1),
    });
    Page::new(lines, paging)
}

fn is_link(value: &Value) -> bool {
    link_href(value).is_some()
}

fn link_href(value: &Value) -> Option<&str> {
    let href = value.as_object()?.get("href")?.as_str()?;
    (href == ROOT_PATH || href.starts_with("/db/")).then_some(href)
}

fn link_text(value: &Value) -> String {
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let id = value.get("id").and_then(scalar_text);
    match (name.is_empty(), id) {
        (false, Some(id)) => format!("{name} (#{id})"),
        (false, None) => name.to_string(),
        (true, Some(id)) => format!("#{id}"),
        (true, None) => link_href(value).unwrap_or_default().to_string(),
    }
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "yes" } else { "no" }.to_string()),
        _ => None,
    }
}

fn humanize(key: &str) -> String {
    key.replace('_', " ")
}

fn push_value(out: &mut Vec<Line>, indent: u8, key: Option<&str>, value: &Value) {
    let label = key.map(humanize);
    if let Some(href) = link_href(value) {
        out.push(Line::link(indent, label, link_text(value), href.to_string()).with_icon(icon_of(value)));
        return;
    }
    match value {
        Value::Null => {}
        Value::Object(map) => {
            if map.is_empty() {
                return;
            }
            let child = match label {
                Some(label) => {
                    out.push(Line::heading(indent, label));
                    indent + 1
                }
                None => indent,
            };
            for (k, v) in map {
                push_value(out, child, Some(k), v);
            }
        }
        Value::Array(items) => push_array(out, indent, label, items),
        scalar => {
            let text = scalar_text(scalar).unwrap_or_default();
            if !text.is_empty() {
                out.push(Line::value(indent, label, text));
            }
        }
    }
}

fn push_array(out: &mut Vec<Line>, indent: u8, label: Option<String>, items: &[Value]) {
    if items.is_empty() {
        return;
    }
    if items.iter().all(|i| scalar_text(i).is_some()) {
        let joined = items.iter().filter_map(scalar_text).collect::<Vec<_>>().join(", ");
        if joined.chars().count() <= JOIN_MAX_CHARS {
            out.push(Line::value(indent, label, joined));
            return;
        }
    }
    let child = match label {
        Some(label) => {
            out.push(Line::heading(indent, format!("{label} ({})", items.len())));
            indent + 1
        }
        None => indent,
    };
    for (n, item) in items.iter().enumerate() {
        if let Some((text, href, icon)) = summarize(item) {
            out.push(Line::link(child, None, text, href).with_icon(icon));
        } else if let Some(text) = scalar_text(item) {
            out.push(Line::value(child, None, text));
        } else {
            push_value(out, child, Some(&format!("#{}", n + 1)), item);
        }
    }
}

/// An object made only of plain values and links collapses to one row: its
/// first link, followed by the other fields (`drop rate: 150`).
fn summarize(value: &Value) -> Option<(String, String, Option<IconRef>)> {
    if let Some(href) = link_href(value) {
        return Some((link_text(value), href.to_string(), icon_of(value)));
    }
    let map = value.as_object()?;
    let mut primary: Option<(String, String, Option<IconRef>)> = None;
    let mut extras = Vec::new();
    for (key, field) in map {
        if let Some(href) = link_href(field) {
            if primary.is_none() {
                primary = Some((link_text(field), href.to_string(), icon_of(field)));
            } else {
                extras.push(format!("{}: {}", humanize(key), link_text(field)));
            }
        } else if let Some(text) = scalar_text(field) {
            if !text.is_empty() {
                extras.push(format!("{}: {text}", humanize(key)));
            }
        } else if !field.is_null() {
            return None;
        }
    }
    let (text, href, icon) = primary?;
    if extras.is_empty() {
        Some((text, href, icon))
    } else {
        Some((format!("{text}   {}", extras.join("   ")), href, icon))
    }
}

/// The picture of what a link points at, for the categories that have one.
fn icon_of(link: &Value) -> Option<IconRef> {
    let href = link_href(link)?;
    let category = category_of(href)?;
    let id = link.get("id").and_then(Value::as_u64).and_then(|id| u16::try_from(id).ok());
    match category {
        "items" => id.map(IconRef::Item),
        "mobs" => id.map(IconRef::Mob),
        "skills" => link.get("name").and_then(Value::as_str).map(|n| IconRef::Skill(n.to_string())),
        _ => None,
    }
}

/// The large picture and name opening an item, monster or skill entry.
fn banner_of(path: &str, entry: &Value) -> Option<Line> {
    let id = entry.get("id").and_then(Value::as_u64).and_then(|id| u16::try_from(id).ok());
    let name = |key: &str| entry.get(key).and_then(Value::as_str).filter(|n| !n.is_empty());
    let icon = match category_of(path)? {
        "items" => IconRef::Item(id?),
        "mobs" => IconRef::Mob(id?),
        "skills" => IconRef::Skill(name("name")?.to_string()),
        _ => return None,
    };
    let title = name("name_english").or_else(|| name("name"))?;
    Some(Line::banner(icon, title.to_string()))
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

    fn requests(events: &[GameEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                GameEvent::RequestReference { path } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    fn frame(win: &mut ReferenceWindow, ctx: UiContext, state: &mut StateCache) -> Vec<GameEvent> {
        let mut ctx = ctx;
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ui = test_frame(&mut ctx, state);
        win.build(&mut ui, &mut BuildCtx::test(&mut character, &data))
    }

    fn categories() -> Value {
        json!({
            "categories": [
                { "name": "items", "href": "/db/items", "count": 12 },
                { "name": "mobs", "href": "/db/mobs", "count": 3 }
            ],
            "wiki": "/wiki/index.md"
        })
    }

    #[test]
    fn categories_become_link_rows_with_counts() {
        let page = build_page("/db", &categories());
        assert_eq!(page.lines.len(), 2);
        assert_eq!(page.lines[0].text, "items  (12)");
        assert_eq!(page.lines[0].link.as_deref(), Some("/db/items"));
    }

    #[test]
    fn list_page_keeps_paging_and_links() {
        let value = json!({
            "total": 230, "offset": 100, "limit": 100,
            "results": [{ "id": 501, "name": "Red Potion", "href": "/db/items/501" }]
        });
        let page = build_page("/db/items?limit=100&offset=100", &value);
        assert_eq!(page.paging, Some(Paging { total: 230, offset: 100, limit: 100 }));
        assert_eq!(page.lines[0].text, "Red Potion (#501)");
        assert_eq!(page.lines[0].link.as_deref(), Some("/db/items/501"));
    }

    #[test]
    fn entry_renders_scalars_nested_objects_and_links() {
        let value = json!({
            "id": 501,
            "name": "Red Potion",
            "weight": 70,
            "empty": null,
            "flags": ["a", "b"],
            "metadata": { "route": "heal", "none": [] },
            "dropped_by": [
                { "mob": { "id": 1002, "name": "Poring", "href": "/db/mobs/1002" }, "rate": 70 },
                { "id": 1113, "name": "Drops", "href": "/db/mobs/1113" }
            ]
        });
        let lines = build_page("/db/items/501", &value).lines;
        let find = |text: &str| lines.iter().find(|l| l.text == text).unwrap_or_else(|| panic!("no row {text:?} in {lines:?}"));

        assert_eq!(
            lines.iter().find(|l| l.label.as_deref() == Some("name")).map(|l| l.text.as_str()),
            Some("Red Potion")
        );
        assert_eq!(find("a, b").label.as_deref(), Some("flags"));
        assert!(lines.iter().any(|l| l.heading && l.text == "metadata"));
        assert_eq!(find("heal").indent, 1);
        assert!(lines.iter().all(|l| l.label.as_deref() != Some("empty")));
        assert!(lines.iter().all(|l| l.text != "none"));

        assert!(lines.iter().any(|l| l.heading && l.text == "dropped by (2)"));
        let drop = find("Poring (#1002)   rate: 70");
        assert_eq!(drop.link.as_deref(), Some("/db/mobs/1002"));
        assert_eq!(find("Drops (#1113)").link.as_deref(), Some("/db/mobs/1113"));
    }

    #[test]
    fn long_value_lists_get_a_row_each() {
        let long: Vec<String> = (0..30).map(|n| format!("value-{n}")).collect();
        let lines = build_page("/db/items/1", &json!({ "tags": long })).lines;
        assert!(lines[0].heading);
        assert_eq!(lines.len(), 31);
    }

    #[test]
    fn nested_objects_without_a_link_get_numbered_headings() {
        let value = json!({ "stages": [{ "a": { "b": 1 } }, { "a": { "b": 2 } }] });
        let lines = build_page("/db/quests/1", &value).lines;
        assert!(lines.iter().any(|l| l.heading && l.text == "#1"));
        assert!(lines.iter().any(|l| l.heading && l.text == "#2"));
    }

    #[test]
    fn query_paths_round_trip() {
        let path = list_path("items", "red potion & more", 0);
        assert_eq!(path, "/db/items?limit=100&q=red%20potion%20%26%20more");
        assert_eq!(query_of(&path), "red potion & more");
        assert_eq!(query_of("/db/items?q=a+b"), "a b");
        assert_eq!(query_of("/db/items"), "");
        assert_eq!(query_of(&list_path("items", "Jellopy é", 0)), "Jellopy é");
    }

    #[test]
    fn offset_is_replaced_not_stacked() {
        let first = list_path("items", "pot", 0);
        let second = with_offset(&first, 100);
        assert!(second.ends_with("&offset=100"));
        assert_eq!(with_offset(&second, 200).matches("offset=").count(), 1);
        assert_eq!(with_offset(&second, 0), first);
        assert_eq!(with_offset("/db/items", 50), "/db/items?offset=50");
    }

    #[test]
    fn paths_are_classified() {
        assert_eq!(shape("/db"), Shape::Categories);
        assert_eq!(shape("/db/items?q=x"), Shape::List);
        assert_eq!(shape("/db/items/501"), Shape::Entry);
        assert_eq!(shape("/db/npcs/prontera/Job%20Master%231"), Shape::Entry);
        assert_eq!(category_of("/db/mobs/PORING"), Some("mobs"));
        assert_eq!(category_of("/db"), None);
    }

    #[test]
    fn opening_asks_for_the_root_once_and_shows_the_answer() {
        let mut win = ReferenceWindow::new();
        let mut state = StateCache::new();
        win.toggle();

        let first = frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&first), vec!["/db".to_string()]);
        let again = frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        assert!(requests(&again).is_empty(), "a pending page must not be asked for twice");

        win.deliver("/db", Ok(categories()));
        let after = frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        assert!(requests(&after).is_empty());
        assert!(matches!(win.pages.get("/db"), Some(Load::Ready(_))));
    }

    #[test]
    fn a_failed_page_is_asked_for_again_after_retry() {
        let mut win = ReferenceWindow::new();
        let mut state = StateCache::new();
        win.toggle();
        frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        win.deliver("/db", Err("connection refused".into()));
        assert!(matches!(win.pages.get("/db"), Some(Load::Failed(_))));

        // The Retry button sits at a fixed spot of the list well.
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 120.0 + PAD + 6.0 + 10.0;
        ctx.mouse_y = 90.0 + TITLE_H + TOOL_H + 26.0 + 10.0;
        ctx.mouse_clicked = true;
        frame(&mut win, ctx, &mut state);
        assert!(!win.pages.contains_key("/db"));

        let events = frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        assert_eq!(requests(&events), vec!["/db".to_string()]);
    }

    #[test]
    fn navigation_keeps_history_and_scroll() {
        let mut win = ReferenceWindow::new();
        win.open_at("/db/items");
        win.scroll = 7;
        win.navigate("/db/items/501".to_string());
        assert_eq!(win.scroll, 0);

        win.go_back();
        assert_eq!(win.current_path(), "/db/items");
        assert_eq!(win.scroll, 7);
        win.go_forward();
        assert_eq!(win.current_path(), "/db/items/501");

        win.go_back();
        win.navigate("/db/mobs".to_string());
        win.go_forward();
        assert_eq!(win.current_path(), "/db/mobs", "a new branch drops the old forward history");
    }

    #[test]
    fn search_targets_the_current_category() {
        let mut win = ReferenceWindow::new();
        win.open_at("/db/mobs/PORING");
        win.search.text = "drops".to_string();
        win.run_search();
        assert_eq!(win.current_path(), "/db/mobs?limit=100&q=drops");
        assert_eq!(win.search.text, "drops");

        win.navigate("/db".to_string());
        win.search.text = "ignored".to_string();
        win.run_search();
        assert_eq!(win.current_path(), "/db", "no category, nothing to search");
    }

    #[test]
    fn clicking_a_link_row_opens_the_entry() {
        let mut win = ReferenceWindow::new();
        let mut state = StateCache::new();
        win.toggle();
        frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        win.deliver("/db", Ok(categories()));

        // The two categories share the first row; "mobs" is the right-hand cell.
        let row_y = 90.0 + TITLE_H + TOOL_H + ROW_H * 0.5;
        let right_cell_x = 120.0 + PAD + (WIN_W - PAD * 2.0 - SCROLLBAR_W) / 2.0 + 30.0;
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = right_cell_x;
        ctx.mouse_y = row_y;
        frame(&mut win, ctx, &mut state);
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = right_cell_x;
        ctx.mouse_y = row_y;
        ctx.mouse_clicked = true;
        frame(&mut win, ctx, &mut state);

        assert_eq!(win.current_path(), "/db/mobs");
    }

    #[test]
    fn npc_paths_encode_the_name() {
        assert_eq!(
            npc_path("prontera.gat", "Job Master#1"),
            "/db/npcs/prontera/Job%20Master%231"
        );
    }

    #[test]
    fn right_clicking_a_link_row_offers_its_entry() {
        let mut win = ReferenceWindow::new();
        let mut state = StateCache::new();
        win.toggle();
        frame(&mut win, UiContext::new(800.0, 600.0), &mut state);
        win.deliver("/db", Ok(categories()));

        let row_y = 90.0 + TITLE_H + TOOL_H + ROW_H * 0.5;
        let right_cell_x = 120.0 + PAD + (WIN_W - PAD * 2.0 - SCROLLBAR_W) / 2.0 + 30.0;
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = right_cell_x;
        ctx.mouse_y = row_y;
        ctx.mouse_right_clicked = true;
        let events = frame(&mut win, ctx, &mut state);

        assert!(
            events.iter().any(|e| matches!(
                e,
                GameEvent::OpenInfoMenu { info: Some(info), describe: None, .. } if info == "/db/mobs"
            )),
            "{events:?}"
        );
        assert_eq!(win.current_path(), "/db", "a right-click does not navigate");
    }

    fn item_data() -> DataTable {
        use ragnarok_game::data_table::item_resource_table::ItemResourceTable;
        let mut data = DataTable::new();
        data.item_resource = Some(ItemResourceTable::from_entries(
            HashMap::from([(501, "red_potion".to_string())]),
            HashMap::new(),
        ));
        data
    }

    fn named_textures(ui: &UiFrame) -> Vec<String> {
        ui.draw_calls
            .iter()
            .filter_map(|call| match &call.texture {
                TextureRef::Named(path) => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn list_rows_and_links_carry_an_icon_by_category() {
        let items = json!({
            "total": 1, "offset": 0, "limit": 100,
            "results": [{ "id": 501, "name": "Red Potion", "href": "/db/items/501" }]
        });
        let lines = build_page("/db/items", &items).lines;
        assert_eq!(lines[0].icon, Some(IconRef::Item(501)));

        let skills = json!({
            "total": 1, "offset": 0, "limit": 100,
            "results": [{ "id": 1, "name": "NV_BASIC", "href": "/db/skills/1" }]
        });
        assert_eq!(build_page("/db/skills", &skills).lines[0].icon, Some(IconRef::Skill("NV_BASIC".into())));

        let maps = json!({
            "total": 1, "offset": 0, "limit": 100,
            "results": [{ "id": null, "name": "prontera", "href": "/db/maps/prontera" }]
        });
        assert_eq!(build_page("/db/maps", &maps).lines[0].icon, None);

        let entry = json!({ "dropped_by": [{ "mob": { "id": 1002, "name": "Poring", "href": "/db/mobs/1002" }, "rate": 7 }] });
        let lines = build_page("/db/items/501", &entry).lines;
        assert_eq!(lines.last().unwrap().icon, Some(IconRef::Mob(1002)));
    }

    #[test]
    fn entries_open_with_a_banner_for_items_mobs_and_skills() {
        let lines = build_page("/db/items/501", &json!({ "id": 501, "name": "Red Potion" })).lines;
        assert!(lines[0].banner);
        assert_eq!(lines[0].text, "Red Potion");
        assert_eq!(lines[0].icon, Some(IconRef::Item(501)));

        let mob = json!({ "id": 1002, "name": "ポリン", "name_english": "Poring" });
        let lines = build_page("/db/mobs/1002", &mob).lines;
        assert_eq!(lines[0].text, "Poring");
        assert_eq!(lines[0].icon, Some(IconRef::Mob(1002)));

        let lines = build_page("/db/maps/prontera", &json!({ "name": "prontera" })).lines;
        assert!(!lines[0].banner);
    }

    #[test]
    fn icon_rows_are_taller_and_scrolling_counts_lines_not_pixels() {
        let plain = Line::value(0, None, "x".into());
        let icon = Line::link(0, None, "x".into(), "/db/items/1".into()).with_icon(Some(IconRef::Item(1)));
        assert_eq!(plain.height(), ROW_H);
        assert_eq!(icon.height(), ROW_H * ICON_ROW_UNITS as f32);

        // 4 rows of 30px (120) + 15 rows of 15px (225) = 345px in a 225px list.
        let mut heights = vec![30.0; 4];
        heights.extend(vec![15.0; 15]);
        let max = max_scroll(&heights);
        assert_eq!(max, 4 + 0, "the last 15 plain rows fill the list exactly");
        assert_eq!(visible_count(&heights, 0), 4 + 7);
        assert_eq!(max_scroll(&[15.0; 3]), 0);
    }

    #[test]
    fn visible_icons_are_requested_once_then_drawn_when_loaded() {
        let mut win = ReferenceWindow::new();
        let mut state = StateCache::new();
        let data = item_data();
        win.open_at("/db/items");
        win.deliver(
            "/db/items",
            Ok(json!({
                "total": 2, "offset": 0, "limit": 100,
                "results": [
                    { "id": 501, "name": "Red Potion", "href": "/db/items/501" },
                    { "id": 999, "name": "Unknown", "href": "/db/items/999" }
                ]
            })),
        );
        let icon = "data/texture/유저인터페이스/item/red_potion.bmp".to_string();

        let build = |win: &mut ReferenceWindow, state: &mut StateCache| {
            let mut ctx = UiContext::new(800.0, 600.0);
            let mut character = Character::new();
            let mut ui = test_frame(&mut ctx, state);
            let events = win.build(&mut ui, &mut BuildCtx::test(&mut character, &data));
            (events, named_textures(&ui))
        };

        let (events, drawn) = build(&mut win, &mut state);
        let asked: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                GameEvent::RequestReferenceTexture { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(asked, vec![icon.as_str()], "item 999 has no icon, so nothing is asked for it");
        assert!(drawn.is_empty());

        let (events, _) = build(&mut win, &mut state);
        assert!(events.iter().all(|e| !matches!(e, GameEvent::RequestReferenceTexture { .. })));

        win.texture_loaded(&icon, Some((24, 24)));
        let (_, drawn) = build(&mut win, &mut state);
        assert_eq!(drawn, vec![icon.clone()]);

        win.texture_loaded(&icon, None);
        let (events, drawn) = build(&mut win, &mut state);
        assert!(drawn.is_empty(), "a picture that failed to load is not drawn");
        assert!(events.iter().all(|e| !matches!(e, GameEvent::RequestReferenceTexture { .. })));
    }

    #[test]
    fn monster_pictures_use_a_sprite_thumbnail_key() {
        use ragnarok_game::data_table::name_table::NameTable;
        let mut data = DataTable::new();
        data.name = Some(NameTable::load());
        let sprite = data.name.as_ref().unwrap().get_name(1002).unwrap().to_string();
        let path = icon_path(&IconRef::Mob(1002), &data).unwrap();
        assert_eq!(path, format!("thumb:data/sprite/몬스터/{sprite}"));
        assert_eq!(icon_path(&IconRef::Mob(60000), &data), None);
        assert_eq!(
            icon_path(&IconRef::Skill("NV_BASIC".into()), &data).unwrap(),
            "data/texture/유저인터페이스/item/nv_basic.bmp"
        );
    }

    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    fn value(label: &str, text: &str) -> Line {
        Line::value(0, Some(label.to_string()), text.to_string())
    }

    #[test]
    fn short_fields_share_a_row_in_pairs() {
        let lines = vec![value("id", "501"), value("name", "Red Potion"), value("type", "Healing")];
        let rows = layout(&measure, &lines, 450.0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells.len(), 2);
        assert_eq!(rows[0].cells[1].text, "Red Potion");
        assert_eq!(rows[1].cells.len(), 1, "an odd one out keeps its own row");
    }

    #[test]
    fn headings_indents_and_kinds_break_a_pair() {
        let nested = Line::value(1, Some("route".into()), "heal".into());
        let link = Line::link(0, None, "Poring".into(), "/db/mobs/1002".into());
        let lines = vec![
            value("a", "1"),
            Line::heading(0, "section".into()),
            value("b", "2"),
            nested,
            link.clone(),
            value("c", "3"),
            link,
        ];
        let rows = layout(&measure, &lines, 450.0);
        assert!(rows.iter().all(|r| r.cells.len() == 1), "{rows:?}");
    }

    #[test]
    fn a_cell_too_wide_for_half_the_list_stays_alone() {
        let wide = value("script", &"x".repeat(40));
        let lines = vec![value("id", "1"), wide, value("id", "2")];
        let rows = layout(&measure, &lines, 450.0);
        assert!(rows.iter().all(|r| r.cells.len() == 1));
    }

    #[test]
    fn long_text_wraps_and_its_pieces_are_not_paired() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon";
        let lines = vec![value("desc", text), value("id", "1")];
        let rows = layout(&measure, &lines, 300.0);
        assert!(rows.len() > 3, "the text takes several rows");
        let pieces: Vec<&str> = rows.iter().take(rows.len() - 1).map(|r| r.cells[0].text.as_str()).collect();
        assert_eq!(pieces.join(" "), text, "nothing is lost or reordered");
        assert_eq!(rows[0].cells[0].label.as_deref(), Some("desc"));
        assert_eq!(rows[1].cells[0].label, None);
        assert!(rows.iter().all(|r| r.cells.len() == 1));
        for row in &rows[..rows.len() - 1] {
            assert!(cell_width(&measure, &row.cells[0]) <= 300.0 - 8.0, "{row:?}");
        }
    }

    #[test]
    fn wrap_cuts_words_wider_than_the_line_and_honours_newlines() {
        let pieces = wrap_text(&measure, "abcdefghijklmnop", 30.0);
        assert!(pieces.len() >= 3 && pieces.iter().all(|p| measure(p) <= 30.0));
        assert_eq!(pieces.concat(), "abcdefghijklmnop");
        assert_eq!(wrap_text(&measure, "one\ntwo", 600.0), vec!["one", "two"]);
        assert_eq!(wrap_text(&measure, "", 100.0), vec![""]);
    }

    #[test]
    fn icon_links_pair_too_and_stay_tall() {
        let a = Line::link(0, None, "Red Potion (#501)".into(), "/db/items/501".into()).with_icon(Some(IconRef::Item(501)));
        let b = Line::link(0, None, "Blue Potion (#505)".into(), "/db/items/505".into()).with_icon(Some(IconRef::Item(505)));
        let rows = layout(&measure, &[a, b], 450.0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].height(), ROW_H * ICON_ROW_UNITS as f32);
    }

    #[test]
    fn escape_closes_the_window() {
        let mut win = ReferenceWindow::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = BuildCtx::test(&mut character, &data);
        assert!(!win.wants_escape(&ctx));
        win.toggle();
        assert!(win.wants_escape(&ctx));
        win.on_escape(&mut ctx);
        assert!(!win.is_open());
    }
}
