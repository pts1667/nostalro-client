//! Windows that server plugins open (rust-ro `docs/plugin-ui.md`). The server sends a tree of nodes; this lays it
//! out each frame (measure, then place), keeps what the player typed, scrolled and selected, and answers clicks,
//! submits, row selections and closes with a `RequestPluginUiEvent` carrying every input's current value.
//!
//! Each open window takes a slot of widget ids: the window itself, its close button, then four ids per node in
//! depth-first order (`Window::nodes`), so a node keeps its ids while the plugin replaces the window.

use std::collections::{BTreeMap, HashMap};

use plugin_ui::{Align, EventKind, InputValue, Node, Table, UiEvent, Window as PluginWindow};
use ragnarok_game::data_table::DataTable;
use ragnarok_game::event::GameEvent;
use ragnarok_renderer::font_atlas::{FontAtlas, bold_char};
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{TextInputBg, UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::text_input::TextInput;
use ragnarok_ui::theme::{FallbackPalette as P, fallback_button};

use crate::BuildCtx;
use crate::helper::fallback;
use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
use crate::helper::window_chrome::{
    draw_hline, draw_sys_button, draw_textured_quad, draw_titlebar, draw_value_bar, text_color,
};

pub const PLUGIN_WINDOW_BASE: u32 = 100000;
const SLOT_SPAN: u32 = 4096;
const SLOTS: u32 = 32;
const CLOSE_OFFSET: u32 = 1;
const FIRST_NODE_OFFSET: u32 = 16;
const IDS_PER_NODE: u32 = 4;

const TITLE_H: f32 = 17.0;
const PAD: f32 = 8.0;
const CLOSE_BTN_SIZE: f32 = 11.0;
const BUTTON_H: f32 = 20.0;
const BUTTON_PAD_X: f32 = 10.0;
const BUTTON_MIN_W: f32 = 40.0;
const INPUT_H: f32 = 18.0;
const ICON_SIZE: f32 = 24.0;
const HEADER_H: f32 = 18.0;
const CELL_PAD: f32 = 2.0;
const GAUGE_H: f32 = 10.0;
const SEPARATOR_H: f32 = 7.0;
const CASCADE: f32 = 24.0;
const NUMBER_MAX_LEN: usize = 11;
const DISABLED_VEIL: [f32; 4] = [1.0, 1.0, 1.0, 0.55];
const SELECTED_ROW: [f32; 4] = [0.20, 0.42, 0.88, 0.35];
const ODD_ROW: [f32; 4] = [0.0, 0.0, 0.0, 0.04];

#[derive(Default)]
pub struct PluginWindows {
    open: Vec<OpenWindow>,
    /// The slot whose input had the keyboard last frame.
    typing_slot: Option<u32>,
}

struct OpenWindow {
    plugin: String,
    window: PluginWindow,
    slot: u32,
    placed: bool,
    /// `elapsed_secs` of the first frame, the start of image animations.
    shown_at: Option<f32>,
    state: WindowState,
}

#[derive(Default)]
struct WindowState {
    fields: HashMap<String, Field>,
    scroll: HashMap<String, usize>,
    selected: HashMap<String, String>,
    widget_rects: Vec<(String, Rect)>,
}

struct Field {
    input: TextInput,
    /// The value the plugin sent last; the player's edit survives a replaced window unless the plugin changes it.
    declared: String,
    number: Option<(i32, i32)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Column,
    Row,
}

impl PluginWindows {
    /// Opens `window`, or replaces the open window of `plugin` with the same id where the player left it.
    pub fn open(&mut self, plugin: &str, window: PluginWindow) {
        if let Some(open) = self
            .open
            .iter_mut()
            .find(|open| open.plugin == plugin && open.window.id == window.id)
        {
            open.state.sync(&window);
            open.window = window;
            return;
        }
        let Some(slot) = (0..SLOTS).find(|slot| self.open.iter().all(|open| open.slot != *slot))
        else {
            tracing::warn!(
                "plugin window {plugin}/{} dropped: {SLOTS} windows are open",
                window.id
            );
            return;
        };
        let mut state = WindowState::default();
        state.sync(&window);
        self.open.push(OpenWindow {
            plugin: plugin.to_string(),
            window,
            slot,
            placed: false,
            shown_at: None,
            state,
        });
    }

    pub fn close(&mut self, plugin: &str, id: &str) {
        self.open
            .retain(|open| !(open.plugin == plugin && open.window.id == id));
    }

    /// Plugin windows belong to the session: the server forgets them when the character leaves.
    pub fn clear(&mut self) {
        self.open.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// An input of a plugin window has the keyboard, so plain keys are text and not shortcuts.
    pub fn is_typing(&self) -> bool {
        self.typing_slot
            .is_some_and(|slot| self.open.iter().any(|open| open.slot == slot))
    }

    pub fn owns(id: WidgetId) -> bool {
        (PLUGIN_WINDOW_BASE..PLUGIN_WINDOW_BASE + SLOTS * SLOT_SPAN).contains(&id.0)
            && (id.0 - PLUGIN_WINDOW_BASE) % SLOT_SPAN == 0
    }

    pub fn window_ids(&self) -> Vec<WidgetId> {
        self.open.iter().map(|open| WidgetId(open.base())).collect()
    }

    /// Every texture the open windows draw: animation frames and item icons, for preloading.
    pub fn textures(&self, data: &DataTable) -> Vec<String> {
        let mut paths = Vec::new();
        for open in &self.open {
            for node in open.window.nodes() {
                match node {
                    Node::Image { image, .. } => {
                        paths.extend(image.frames.iter().map(|frame| texture_path(frame)))
                    }
                    Node::ItemIcon { item_id, .. } => paths.extend(item_icon(data, *item_id)),
                    _ => {}
                }
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// Escape on the window `id`: a closable window closes and tells its plugin.
    pub fn escape(&mut self, id: WidgetId) -> Option<Vec<GameEvent>> {
        let index = self
            .open
            .iter()
            .position(|open| open.base() == id.0 && open.window.closable)?;
        let open = self.open.remove(index);
        Some(vec![closed_event(&open)])
    }

    /// Where a widget was drawn last frame, by plugin, window and widget id.
    pub fn widget_rect(&self, plugin: &str, window: &str, widget: &str) -> Option<Rect> {
        let open = self
            .open
            .iter()
            .find(|open| open.plugin == plugin && open.window.id == window)?;
        open.state
            .widget_rects
            .iter()
            .find(|(id, _)| id == widget)
            .map(|(_, rect)| *rect)
    }

    pub fn build_window(
        &mut self,
        id: WidgetId,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
    ) -> Vec<GameEvent> {
        let Some(index) = self.open.iter().position(|open| open.base() == id.0) else {
            return Vec::new();
        };
        let slot = self.open[index].slot;
        let (events, closed, typing) = self.open[index].build(ui, ctx);
        if typing {
            self.typing_slot = Some(slot);
        } else if self.typing_slot == Some(slot) {
            self.typing_slot = None;
        }
        if closed {
            self.open.remove(index);
        }
        events
    }
}

impl OpenWindow {
    fn base(&self) -> u32 {
        PLUGIN_WINDOW_BASE + self.slot * SLOT_SPAN
    }

    fn size(&self, atlas: &FontAtlas, screen_h: f32) -> (f32, f32) {
        let width = f32::from(self.window.width);
        let height = match self.window.height {
            Some(height) => f32::from(height),
            None => {
                TITLE_H
                    + 2.0 * PAD
                    + measure(atlas, &self.window.node, width - 2.0 * PAD, Axis::Column).1
            }
        };
        (width, height.min(screen_h.max(TITLE_H)))
    }

    /// The events of the frame, whether the player closed the window, and whether one of its inputs has the keyboard.
    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> (Vec<GameEvent>, bool, bool) {
        let window_id = WidgetId(self.base());
        let (width, height) = self.size(ui.atlas, ui.ctx.screen_height);
        if !self.placed {
            let cascade = self.slot as f32 * CASCADE;
            let x = ((ui.ctx.screen_width - width) / 2.0).floor() + cascade;
            let y = ((ui.ctx.screen_height - height) / 3.0).floor() + cascade;
            ui.set_window_position(window_id, x, y);
            self.placed = true;
        }
        let seconds = ui.elapsed_secs - *self.shown_at.get_or_insert(ui.elapsed_secs);
        let win = ui.window_at(window_id, width, height, TITLE_H, 0.0, 0.0);
        ui.interact(window_id, win);

        draw_titlebar(ui, win.x, win.y, width, TITLE_H);
        ui.text(
            win.x + 16.0,
            win.y + TITLE_H - 3.0,
            &self.window.title,
            text_color(),
        );
        fallback::window_body(ui, win.x, win.y + TITLE_H, width, height - TITLE_H);

        let mut closed = false;
        if self.window.closable {
            let close_rect = Rect::new(
                win.x + width - CLOSE_BTN_SIZE - 3.0,
                win.y + (TITLE_H - CLOSE_BTN_SIZE) / 2.0,
                CLOSE_BTN_SIZE,
                CLOSE_BTN_SIZE,
            );
            let response = ui.interact(WidgetId(self.base() + CLOSE_OFFSET), close_rect);
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            draw_sys_button(
                ui,
                close_rect,
                (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
                response.hovered(),
                Some('x'),
            );
            closed = response.clicked();
        }

        self.state.widget_rects.clear();
        let mut painter = Painter {
            window_id: &self.window.id,
            base: self.base(),
            seconds,
            state: &mut self.state,
            events: Vec::new(),
            widget_clicked: false,
            typing: false,
        };
        let avail = width - 2.0 * PAD;
        let (_, content_h) = measure(ui.atlas, &self.window.node, avail, Axis::Column);
        let mut index = 0;
        painter.draw(
            ui,
            ctx.data,
            &self.window.node,
            &mut index,
            Rect::new(win.x + PAD, win.y + TITLE_H + PAD, avail, content_h),
            avail,
            Axis::Column,
        );

        let typing = painter.typing;
        let mut events: Vec<GameEvent> = painter
            .events
            .into_iter()
            .map(|event| GameEvent::RequestPluginUiEvent {
                plugin: self.plugin.clone(),
                event,
            })
            .collect();
        if closed {
            events.push(closed_event(self));
        }
        (events, closed, typing)
    }
}

impl WindowState {
    /// Keeps what the player changed in the inputs, tables and selections that the new `window` still has.
    fn sync(&mut self, window: &PluginWindow) {
        let mut fields = HashMap::new();
        let mut tables = HashMap::new();
        for node in window.nodes() {
            match node {
                Node::NumberInput {
                    id,
                    value,
                    min,
                    max,
                    ..
                } => {
                    let declared = value.to_string();
                    let field = self.take_field(id, &declared).unwrap_or_else(|| {
                        Field::new(&declared, NUMBER_MAX_LEN, Some((*min, *max)))
                    });
                    fields.insert(
                        id.clone(),
                        Field {
                            number: Some((*min, *max)),
                            ..field
                        },
                    );
                }
                Node::TextInput {
                    id, value, max_len, ..
                } => {
                    let field = self
                        .take_field(id, value)
                        .unwrap_or_else(|| Field::new(value, usize::from(*max_len), None));
                    fields.insert(id.clone(), field);
                }
                Node::Table(table) => {
                    tables.insert(table.id.clone(), table);
                }
                _ => {}
            }
        }
        self.fields = fields;
        self.scroll.retain(|id, _| tables.contains_key(id));
        self.selected.retain(|id, row| {
            tables
                .get(id)
                .is_some_and(|table| table.selectable && table.rows.iter().any(|r| &r.id == row))
        });
    }

    fn take_field(&mut self, id: &str, declared: &str) -> Option<Field> {
        self.fields
            .remove(id)
            .filter(|field| field.declared == declared)
    }

    fn inputs(&self) -> BTreeMap<String, InputValue> {
        self.fields
            .iter()
            .map(|(id, field)| (id.clone(), field.value()))
            .collect()
    }
}

impl Field {
    fn new(value: &str, max_len: usize, number: Option<(i32, i32)>) -> Self {
        let mut input = TextInput::new(max_len.max(1), false).with_numeric_only(number.is_some());
        input.text = value.to_string();
        input.move_cursor_to_end();
        Self {
            input,
            declared: value.to_string(),
            number,
        }
    }

    fn value(&self) -> InputValue {
        match self.number {
            Some((min, max)) => {
                let typed = self
                    .input
                    .text
                    .parse()
                    .or_else(|_| self.declared.parse())
                    .unwrap_or(min);
                InputValue::Number(typed.clamp(min, max.max(min)))
            }
            None => InputValue::Text(self.input.text.clone()),
        }
    }
}

struct Painter<'w> {
    window_id: &'w str,
    base: u32,
    seconds: f32,
    state: &'w mut WindowState,
    events: Vec<UiEvent>,
    /// A button took this frame's click, so the table row under it is not selected too.
    widget_clicked: bool,
    typing: bool,
}

impl Painter<'_> {
    fn ids(&self, index: u32) -> [WidgetId; 4] {
        let first = self.base + FIRST_NODE_OFFSET + index * IDS_PER_NODE;
        [
            WidgetId(first),
            WidgetId(first + 1),
            WidgetId(first + 2),
            WidgetId(first + 3),
        ]
    }

    fn emit(&mut self, kind: EventKind, widget: &str, row: Option<String>) {
        self.events.push(UiEvent {
            window: self.window_id.to_string(),
            widget: widget.to_string(),
            kind,
            row,
            inputs: self.state.inputs(),
        });
    }

    /// Draws `node` in `rect` (its measured size, or the parent's width for text and separators); `avail` is the
    /// width it was measured against.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        ui: &mut UiFrame,
        data: &DataTable,
        node: &Node,
        index: &mut u32,
        rect: Rect,
        avail: f32,
        axis: Axis,
    ) {
        let ids = self.ids(*index);
        *index += 1;
        let atlas = ui.atlas;
        match node {
            Node::Column(column) => {
                let mut y = rect.y;
                for child in &column.children {
                    let (w, h) = measure(atlas, child, avail, Axis::Column);
                    let fills = matches!(child, Node::Text { .. } | Node::Separator);
                    let (x, w) = if fills {
                        (rect.x, avail)
                    } else {
                        (rect.x + aligned(column.align, avail - w), w)
                    };
                    self.draw(
                        ui,
                        data,
                        child,
                        index,
                        Rect::new(x, y, w, h),
                        avail,
                        Axis::Column,
                    );
                    y += h + f32::from(column.gap);
                }
            }
            Node::Row(row) => {
                let mut x = rect.x;
                for child in &row.children {
                    let (w, h) = measure(atlas, child, avail, Axis::Row);
                    let y = rect.y + aligned(row.align, rect.h - h);
                    self.draw(
                        ui,
                        data,
                        child,
                        index,
                        Rect::new(x, y, w, h),
                        avail,
                        Axis::Row,
                    );
                    x += w + f32::from(row.gap);
                }
            }
            Node::Spacer { .. } => {}
            Node::Separator => match axis {
                Axis::Column => draw_hline(ui, rect.x, rect.y + SEPARATOR_H / 2.0, rect.w),
                Axis::Row => fill(
                    ui,
                    Rect::new(rect.x + SEPARATOR_H / 2.0, rect.y, 1.0, rect.h),
                    P::WIN_BORDER,
                ),
            },
            Node::Text {
                text,
                color,
                bold,
                align,
                wrap,
            } => {
                let color = color
                    .as_deref()
                    .and_then(parse_rgb)
                    .unwrap_or_else(text_color);
                let line_h = atlas.line_height;
                for (i, line) in text_lines(atlas, text, *bold, *wrap, avail)
                    .iter()
                    .enumerate()
                {
                    let top = rect.y + i as f32 * line_h;
                    let x = rect.x + aligned(*align, rect.w - text_width(atlas, line, *bold));
                    let baseline = baseline(atlas, top, line_h);
                    if *bold {
                        ui.text_bold(x, baseline, line, color);
                    } else {
                        ui.colored_text(x, baseline, line, color);
                    }
                }
            }
            Node::Image { image, .. } => {
                if let Some(frame) = image.frame_at(self.seconds) {
                    draw_textured_quad(ui, rect.x, rect.y, rect.w, rect.h, &texture_path(frame));
                }
            }
            Node::ItemIcon { item_id, amount } => {
                fallback::slot_cell(ui, rect.x, rect.y, rect.w, rect.h);
                if let Some(path) = item_icon(data, *item_id) {
                    draw_textured_quad(ui, rect.x, rect.y, rect.w, rect.h, &path);
                }
                if let Some(amount) = amount {
                    let label = amount.to_string();
                    let x = rect.x + rect.w - atlas.measure_text(&label);
                    ui.text(x, rect.y + rect.h - 2.0, &label, text_color());
                }
                if ui.interact(ids[0], rect).hovered() {
                    let name = data
                        .item_name
                        .as_ref()
                        .map(|names| names.get_name_or_id(*item_id as u16))
                        .unwrap_or_else(|| item_id.to_string());
                    ui.tooltip(rect.x + rect.w, rect.y, &name);
                }
            }
            Node::Button { id, label, enabled } => {
                self.state.widget_rects.push((id.clone(), rect));
                if *enabled {
                    if ui.button(ids[0], rect, label).clicked() {
                        self.widget_clicked = true;
                        self.emit(EventKind::Click, id, None);
                    }
                } else {
                    fallback_button(ui, rect, false, false, label);
                    fill(ui, rect, DISABLED_VEIL);
                }
            }
            Node::NumberInput { id, .. } | Node::TextInput { id, .. } => {
                self.state.widget_rects.push((id.clone(), rect));
                let Some(field) = self.state.fields.get_mut(id) else {
                    return;
                };
                let response = ui.text_input(ids[0], rect, &mut field.input, TextInputBg::Default);
                self.typing |= response.has_focus();
                if response.has_focus() && ui.ctx.key_enter {
                    self.emit(EventKind::Submit, id, None);
                }
            }
            Node::Gauge {
                value, max, color, ..
            } => {
                let ratio = if *max == 0 {
                    0.0
                } else {
                    *value as f32 / *max as f32
                };
                let fill_color = color
                    .as_deref()
                    .and_then(parse_rgb)
                    .unwrap_or(crate::helper::window_chrome::EXP_BAR_FILL);
                draw_value_bar(
                    ui,
                    rect.x,
                    rect.y,
                    rect.w - 2.0,
                    rect.h - 2.0,
                    ratio,
                    fill_color,
                );
            }
            Node::Table(table) => self.draw_table(ui, data, table, ids, index, rect),
        }
    }

    fn draw_table(
        &mut self,
        ui: &mut UiFrame,
        data: &DataTable,
        table: &Table,
        ids: [WidgetId; 4],
        index: &mut u32,
        rect: Rect,
    ) {
        let atlas = ui.atlas;
        self.state.widget_rects.push((table.id.clone(), rect));
        let columns_w = columns_width(table);
        let row_h = f32::from(table.row_height);
        let visible = usize::from(table.visible_rows.max(1));
        let shown = visible.min(table.rows.len()).max(1);
        let max_scroll = table.rows.len().saturating_sub(visible);
        let offset = self
            .state
            .scroll
            .get(&table.id)
            .copied()
            .unwrap_or(0)
            .min(max_scroll);

        fill(
            ui,
            Rect::new(rect.x, rect.y, rect.w, HEADER_H),
            P::WIN_BODY_ALT,
        );
        let mut x = rect.x;
        for column in &table.columns {
            let width = f32::from(column.width);
            let title_x = x
                + CELL_PAD
                + aligned(
                    column.align,
                    width - 2.0 * CELL_PAD - text_width(atlas, &column.title, true),
                );
            ui.text_bold(
                title_x,
                baseline(atlas, rect.y, HEADER_H),
                &column.title,
                text_color(),
            );
            x += width;
        }
        draw_hline(ui, rect.x, rect.y + HEADER_H - 1.0, rect.w);

        let body = Rect::new(rect.x, rect.y + HEADER_H, columns_w, shown as f32 * row_h);
        let selected = self.state.selected.get(&table.id).cloned();
        let clicked_before = std::mem::replace(&mut self.widget_clicked, false);
        for (number, row) in table.rows.iter().enumerate() {
            if number < offset || number >= offset + visible {
                *index += row.cells.iter().map(count_nodes).sum::<u32>();
                continue;
            }
            let top = body.y + (number - offset) as f32 * row_h;
            let row_rect = Rect::new(body.x, top, columns_w, row_h);
            if selected.as_deref() == Some(row.id.as_str()) {
                fill(ui, row_rect, SELECTED_ROW);
            } else if number % 2 == 1 {
                fill(ui, row_rect, ODD_ROW);
            }
            let mut x = body.x;
            for (column, cell) in row.cells.iter().enumerate() {
                let Some(column) = table.columns.get(column) else {
                    *index += count_nodes(cell);
                    continue;
                };
                let width = f32::from(column.width) - 2.0 * CELL_PAD;
                let (w, h) = measure(atlas, cell, width, Axis::Row);
                let cell_rect = Rect::new(
                    x + CELL_PAD + aligned(column.align, width - w),
                    top + (row_h - h) / 2.0,
                    w.min(width),
                    h,
                );
                self.draw(ui, data, cell, index, cell_rect, width, Axis::Row);
                x += f32::from(column.width);
            }
        }

        if table.selectable {
            let response = ui.interact(ids[3], body);
            if response.clicked() && !self.widget_clicked {
                let number = offset + ((ui.ctx.mouse_y - body.y) / row_h).max(0.0) as usize;
                if let Some(row) = table.rows.get(number) {
                    self.state.selected.insert(table.id.clone(), row.id.clone());
                    self.emit(EventKind::Select, &table.id, Some(row.id.clone()));
                }
            }
        }
        self.widget_clicked |= clicked_before;

        if table.rows.len() > visible {
            let offset = scrollbar::scrollbar(
                ui,
                ScrollbarIds {
                    up: ids[0],
                    down: ids[1],
                    thumb: ids[2],
                },
                offset,
                visible,
                max_scroll,
                Rect::new(rect.x, body.y, rect.w, body.h),
                rect.x + columns_w,
                body.y,
                body.h,
            );
            self.state.scroll.insert(table.id.clone(), offset);
        }
    }
}

/// Width and height of `node` laid out in `avail` pixels of width, inside a column or a row.
fn measure(atlas: &FontAtlas, node: &Node, avail: f32, axis: Axis) -> (f32, f32) {
    match node {
        Node::Column(column) => {
            let sizes: Vec<_> = column
                .children
                .iter()
                .map(|child| measure(atlas, child, avail, Axis::Column))
                .collect();
            let gaps = f32::from(column.gap) * sizes.len().saturating_sub(1) as f32;
            (
                sizes.iter().map(|size| size.0).fold(0.0, f32::max),
                sizes.iter().map(|size| size.1).sum::<f32>() + gaps,
            )
        }
        Node::Row(row) => {
            let sizes: Vec<_> = row
                .children
                .iter()
                .map(|child| measure(atlas, child, avail, Axis::Row))
                .collect();
            let gaps = f32::from(row.gap) * sizes.len().saturating_sub(1) as f32;
            (
                sizes.iter().map(|size| size.0).sum::<f32>() + gaps,
                sizes.iter().map(|size| size.1).fold(0.0, f32::max),
            )
        }
        Node::Spacer { size } => match axis {
            Axis::Column => (0.0, f32::from(*size)),
            Axis::Row => (f32::from(*size), 0.0),
        },
        Node::Separator => match axis {
            Axis::Column => (0.0, SEPARATOR_H),
            Axis::Row => (SEPARATOR_H, 0.0),
        },
        Node::Text {
            text, bold, wrap, ..
        } => {
            let lines = text_lines(atlas, text, *bold, *wrap, avail);
            let width = lines
                .iter()
                .map(|line| text_width(atlas, line, *bold))
                .fold(0.0, f32::max);
            (width, lines.len() as f32 * atlas.line_height)
        }
        Node::Image { width, height, .. } => (f32::from(*width), f32::from(*height)),
        Node::ItemIcon { .. } => (ICON_SIZE, ICON_SIZE),
        Node::Button { label, .. } => (
            (atlas.measure_text(label) + 2.0 * BUTTON_PAD_X).max(BUTTON_MIN_W),
            BUTTON_H,
        ),
        Node::NumberInput { width, .. } | Node::TextInput { width, .. } => {
            (f32::from(*width), INPUT_H)
        }
        Node::Gauge { width, .. } => (f32::from(*width), GAUGE_H),
        Node::Table(table) => {
            let visible = usize::from(table.visible_rows.max(1));
            let scrollbar = if table.rows.len() > visible {
                SCROLLBAR_W
            } else {
                0.0
            };
            let shown = visible.min(table.rows.len()).max(1);
            (
                columns_width(table) + scrollbar,
                HEADER_H + shown as f32 * f32::from(table.row_height),
            )
        }
    }
}

fn columns_width(table: &Table) -> f32 {
    table
        .columns
        .iter()
        .map(|column| f32::from(column.width))
        .sum()
}

fn count_nodes(node: &Node) -> u32 {
    1 + node.children().into_iter().map(count_nodes).sum::<u32>()
}

/// The lines of a text node. Wrapped lines carry the colour code that was active where the previous one ended; bold
/// text is drawn in one colour, so its codes are dropped.
fn text_lines(atlas: &FontAtlas, text: &str, bold: bool, wrap: bool, width: f32) -> Vec<String> {
    let text = if bold {
        draw::strip_color_codes(text)
    } else {
        text.to_string()
    };
    if !wrap {
        return text.split('\n').map(str::to_string).collect();
    }
    let mut carried: Option<String> = None;
    let mut lines = Vec::new();
    for line in draw::word_wrap(
        &text,
        width.max(1.0),
        |line| text_width(atlas, line, bold),
        false,
    ) {
        let shown = match &carried {
            Some(code) => format!("{code}{line}"),
            None => line.clone(),
        };
        if let Some(code) = last_color_code(&line) {
            carried = Some(code);
        }
        lines.push(shown);
    }
    lines
}

fn last_color_code(line: &str) -> Option<String> {
    line.match_indices('^')
        .filter_map(|(at, _)| line.get(at..at + 7))
        .filter(|code| code[1..].bytes().all(|b| b.is_ascii_hexdigit()))
        .last()
        .map(str::to_string)
}

fn text_width(atlas: &FontAtlas, line: &str, bold: bool) -> f32 {
    let plain = draw::strip_color_codes(line);
    if bold {
        atlas.measure_text(&plain.chars().map(bold_char).collect::<String>())
    } else {
        atlas.measure_text(&plain)
    }
}

fn baseline(atlas: &FontAtlas, top: f32, height: f32) -> f32 {
    top + (height + atlas.ascent) / 2.0
}

fn aligned(align: Align, free: f32) -> f32 {
    match align {
        Align::Start => 0.0,
        Align::Center => (free / 2.0).max(0.0).floor(),
        Align::End => free.max(0.0),
    }
}

fn fill(ui: &mut UiFrame, rect: Rect, color: [f32; 4]) {
    let (vertices, indices) = draw::quad_vertices(rect.x, rect.y, rect.w, rect.h, color);
    ui.draw_calls.push(DrawCall {
        vertices: vertices.to_vec(),
        indices: indices.to_vec(),
        texture: TextureRef::White,
    });
}

/// `RRGGBB` as sent by plugins.
pub fn parse_rgb(hex: &str) -> Option<[f32; 4]> {
    let value = u32::from_str_radix(hex.get(..6)?, 16).ok()?;
    Some(crate::helper::colors::rgb(value))
}

/// Plugin frame paths are relative to the client's `data/` folder (`plugin/<name>/coin.png`, `texture/...`).
pub fn texture_path(frame: &str) -> String {
    format!("data/{frame}")
}

fn item_icon(data: &DataTable, item_id: u32) -> Option<String> {
    data.item_resource
        .as_ref()?
        .item_icon_path(u16::try_from(item_id).ok()?)
}

fn closed_event(open: &OpenWindow) -> GameEvent {
    GameEvent::RequestPluginUiEvent {
        plugin: open.plugin.clone(),
        event: UiEvent {
            window: open.window.id.clone(),
            widget: String::new(),
            kind: EventKind::Closed,
            row: None,
            inputs: BTreeMap::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use plugin_ui::{ImageRef, Window};
    use ragnarok_game::character::Character;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::TestFrame;

    use super::*;

    const SHOP: &str = "ui_shop";

    fn shop(amount: i32) -> Window {
        let mut table = Table::new("stock", 2)
            .column("", 32, Align::Center)
            .column("Item", 100, Align::Start)
            .column("", 56, Align::Center)
            .selectable();
        for (item, enabled) in [(501, true), (502, false), (503, true)] {
            let buy = Node::button(format!("buy:{item}"), "Buy");
            table = table.row(
                item.to_string(),
                [
                    Node::item(item),
                    Node::text(format!("Potion {item}")),
                    if enabled { buy } else { buy.disabled() },
                ],
            );
        }
        Window::new(
            "shop",
            "Shop",
            260,
            Node::column([
                Node::row([
                    Node::image(
                        ImageRef::animated(
                            ["plugin/ui_shop/coin_0.png", "plugin/ui_shop/coin_1.png"],
                            4.0,
                        ),
                        24,
                        24,
                    ),
                    Node::title("100 zeny"),
                ]),
                Node::wrapped("^ff0000Welcome^000000 to the shop, take your time and look around."),
                table.into(),
                Node::row([
                    Node::text("Amount"),
                    Node::number_input("amount", amount, 1, 50),
                    Node::button("close", "Close"),
                ]),
            ]),
        )
    }

    /// Builds every open window for one frame, with the pointer at `click` when given.
    fn frame(
        windows: &mut PluginWindows,
        state: &mut StateCache,
        click: Option<(f32, f32)>,
        typed: &str,
    ) -> Vec<GameEvent> {
        let mut ctx = UiContext::new(800.0, 600.0);
        if let Some((x, y)) = click {
            ctx.mouse_x = x;
            ctx.mouse_y = y;
            ctx.mouse_clicked = true;
        }
        ctx.typed_chars = typed.chars().collect();
        let mut ui = TestFrame::new().elapsed(1.0).build(&mut ctx, state);
        let data = DataTable::new();
        let mut character = Character::new();
        let mut build = BuildCtx::test(&mut character, &data);
        let mut events = Vec::new();
        for id in windows.window_ids() {
            events.extend(windows.build_window(id, &mut ui, &mut build));
        }
        events
    }

    fn center(rect: Rect) -> (f32, f32) {
        (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
    }

    fn ui_event(event: &GameEvent) -> &UiEvent {
        match event {
            GameEvent::RequestPluginUiEvent { plugin, event } if plugin == SHOP => event,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_click_on_buy_carries_the_amount_and_disabled_buttons_stay_silent() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        windows.open(SHOP, shop(1));
        assert!(frame(&mut windows, &mut state, None, "").is_empty());

        let amount = windows.widget_rect(SHOP, "shop", "amount").unwrap();
        frame(&mut windows, &mut state, Some(center(amount)), "");
        frame(&mut windows, &mut state, None, "2");
        assert!(windows.is_typing());
        let buy = windows.widget_rect(SHOP, "shop", "buy:501").unwrap();
        let events = frame(&mut windows, &mut state, Some(center(buy)), "");
        assert!(
            !windows.is_typing(),
            "the click moved the focus to the button"
        );
        assert_eq!(
            events.len(),
            1,
            "the row under the button is not selected too: {events:?}"
        );
        let event = ui_event(&events[0]);
        assert_eq!(
            (event.kind, event.widget.as_str()),
            (EventKind::Click, "buy:501")
        );
        assert_eq!(
            event.inputs["amount"],
            InputValue::Number(12),
            "typed after the declared 1, within 1..=50"
        );

        let disabled = windows.widget_rect(SHOP, "shop", "buy:502").unwrap();
        let events = frame(&mut windows, &mut state, Some(center(disabled)), "");
        assert!(
            events
                .iter()
                .all(|event| ui_event(event).kind == EventKind::Select),
            "{events:?}"
        );
    }

    #[test]
    fn numbers_are_clamped_and_a_replaced_window_keeps_the_edit_unless_the_plugin_changes_it() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        windows.open(SHOP, shop(1));
        frame(&mut windows, &mut state, None, "");
        let amount = windows.widget_rect(SHOP, "shop", "amount").unwrap();
        frame(&mut windows, &mut state, Some(center(amount)), "");
        frame(&mut windows, &mut state, None, "999");
        let close = windows.widget_rect(SHOP, "shop", "close").unwrap();
        let events = frame(&mut windows, &mut state, Some(center(close)), "");
        assert_eq!(
            ui_event(&events[0]).inputs["amount"],
            InputValue::Number(50)
        );

        windows.open(SHOP, shop(1));
        let events = frame(&mut windows, &mut state, Some(center(close)), "");
        assert_eq!(
            ui_event(&events[0]).inputs["amount"],
            InputValue::Number(50),
            "same declared value: the edit stays"
        );
        windows.open(SHOP, shop(3));
        let events = frame(&mut windows, &mut state, Some(center(close)), "");
        assert_eq!(
            ui_event(&events[0]).inputs["amount"],
            InputValue::Number(3),
            "the plugin set a new value"
        );
    }

    #[test]
    fn rows_select_and_tables_scroll() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        windows.open(SHOP, shop(1));
        frame(&mut windows, &mut state, None, "");
        let table = windows.widget_rect(SHOP, "shop", "stock").unwrap();
        let second_row = (table.x + 60.0, table.y + HEADER_H + 24.0 + 12.0);
        let events = frame(&mut windows, &mut state, Some(second_row), "");
        let event = ui_event(&events[0]);
        assert_eq!(
            (event.kind, event.row.as_deref()),
            (EventKind::Select, Some("502"))
        );

        assert!(
            windows.widget_rect(SHOP, "shop", "buy:503").is_none(),
            "two visible rows of three"
        );
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = table.x + 60.0;
        ctx.mouse_y = table.y + HEADER_H + 10.0;
        ctx.scroll_delta = -1.0;
        let mut ui = TestFrame::new().build(&mut ctx, &mut state);
        let data = DataTable::new();
        let mut character = Character::new();
        for id in windows.window_ids() {
            windows.build_window(id, &mut ui, &mut BuildCtx::test(&mut character, &data));
        }
        drop(ui);
        frame(&mut windows, &mut state, None, "");
        assert!(windows.widget_rect(SHOP, "shop", "buy:503").is_some());
        assert!(windows.widget_rect(SHOP, "shop", "buy:501").is_none());
    }

    #[test]
    fn closing_tells_the_plugin_and_escape_closes_only_closable_windows() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        windows.open(SHOP, shop(1));
        windows.open(
            SHOP,
            Window::new("pinned", "Pinned", 120, Node::text("Stay")).fixed(),
        );
        frame(&mut windows, &mut state, None, "");
        let ids = windows.window_ids();
        assert!(ids.iter().all(|id| PluginWindows::owns(*id)));
        assert!(
            !PluginWindows::owns(WidgetId(ids[0].0 + 1)),
            "a close button is not a window"
        );
        assert!(windows.escape(ids[1]).is_none());
        let events = windows.escape(ids[0]).unwrap();
        let event = ui_event(&events[0]);
        assert_eq!(
            (event.kind, event.window.as_str()),
            (EventKind::Closed, "shop")
        );
        assert_eq!(windows.window_ids(), vec![ids[1]]);

        windows.open("other", shop(1));
        assert_eq!(windows.window_ids()[1], ids[0], "the freed slot is reused");
        windows.close("other", "shop");
        windows.close(SHOP, "pinned");
        assert!(windows.is_empty());
    }

    #[test]
    fn textures_list_frames_under_data_and_wrapped_text_carries_its_colour() {
        let mut windows = PluginWindows::default();
        windows.open(SHOP, shop(1));
        assert_eq!(
            windows.textures(&DataTable::new()),
            vec![
                "data/plugin/ui_shop/coin_0.png",
                "data/plugin/ui_shop/coin_1.png"
            ]
        );
        let atlas = ragnarok_ui::test_support::atlas();
        let lines = text_lines(
            atlas,
            "^ff0000red red red red red red red red red red",
            false,
            true,
            60.0,
        );
        assert!(lines.len() > 1);
        assert!(
            lines.iter().all(|line| line.starts_with("^ff0000")),
            "{lines:?}"
        );
        assert_eq!(
            parse_rgb("00a000"),
            Some(crate::helper::colors::rgb(0x00a000))
        );
        assert_eq!(parse_rgb("zz"), None);
    }

    #[test]
    fn a_window_grows_with_its_content_and_spacers_follow_their_parent() {
        let atlas = ragnarok_ui::test_support::atlas();
        let row = Node::row([Node::text("a"), Node::spacer(90), Node::button("b", "B")]);
        let (w, h) = measure(atlas, &row, 300.0, Axis::Column);
        assert!(w > 90.0 + BUTTON_MIN_W);
        assert_eq!(h, BUTTON_H, "a spacer in a row is only wide");
        let column = Node::column([Node::spacer(30), Node::text("a")]).gap(0);
        assert_eq!(
            measure(atlas, &column, 300.0, Axis::Column).1,
            30.0 + atlas.line_height
        );
    }
}
