//! Windows that server plugins open (rust-ro `docs/plugin-ui.md`). The server sends a tree of nodes; this lays it
//! out each frame (measure, then place), keeps what the player typed, scrolled and selected, and answers clicks,
//! submits, row selections and closes with a `RequestPluginUiEvent` carrying every input's current value.
//!
//! Each open window takes a slot of widget ids: the window itself, its close button, then four ids per node in
//! depth-first order (`Window::nodes`), so a node keeps its ids while the plugin replaces the window.

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use plugin_ui::{
    Align, EventKind, Frame, InputValue, Node, Placement, Table, UiEvent, Window as PluginWindow,
    format_countdown,
};
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
/// Gap between HUD windows that stack in one slot, and the slot's margin from the screen's right edge.
const HUD_STACK_GAP: f32 = 4.0;
/// The plugin name of the windows the client opens on its own, unlike any server plugin's.
const OWN_PLUGIN: &str = "@client";
const HUD_MARGIN: f32 = 2.0;
/// The widget after the close button: the collapse toggle, or the header line of a frameless window.
const COLLAPSE_OFFSET: u32 = 2;
const HEADER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// The dark ring around outlined text, one pixel out in each direction.
const OUTLINE_OFFSETS: [(f32, f32); 8] = [
    (-1.0, -1.0),
    (0.0, -1.0),
    (1.0, -1.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, 1.0),
    (0.0, 1.0),
    (1.0, 1.0),
];

/// Where a unit is on screen this frame: its centre, and the top of its sprite.
#[derive(Clone, Copy)]
pub struct UnitAnchor {
    pub x: f32,
    pub top: f32,
}

#[derive(Default)]
pub struct PluginWindows {
    open: Vec<OpenWindow>,
    /// The slot whose input had the keyboard last frame.
    typing_slot: Option<u32>,
    /// Top of the HUD slot below the minimap, set each frame by the client.
    hud_top: f32,
    /// Units of the anchored windows that are on screen, set each frame.
    unit_anchors: HashMap<u32, UnitAnchor>,
}

struct OpenWindow {
    plugin: String,
    window: PluginWindow,
    slot: u32,
    placed: bool,
    /// `elapsed_secs` of the first frame, the start of image animations.
    shown_at: Option<f32>,
    /// When the window last arrived, the start of its countdowns; a re-sent window starts them again.
    arrived: Instant,
    /// Folded down to its title by the player; kept across re-opens of the id.
    collapsed: bool,
    /// An anchored window whose unit the client does not know yet, logged once until the unit shows up.
    waiting_for_unit: bool,
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
        tracing::info!(
            "plugin window {plugin}/{} opened ({:?})",
            window.id,
            window.placement
        );
        if let Some(open) = self
            .open
            .iter_mut()
            .find(|open| open.plugin == plugin && open.window.id == window.id)
        {
            open.state.sync(&window);
            open.window = window;
            open.arrived = Instant::now();
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
            arrived: Instant::now(),
            collapsed: false,
            waiting_for_unit: false,
            state,
        });
    }

    /// Shows the client's own window `id`, ahead of the plugins' windows of its HUD slot, or hides it with
    /// `None`. A window equal to the one shown is left alone, so the client calls this every frame.
    pub fn show_own(&mut self, id: &str, window: Option<PluginWindow>) {
        let index = self
            .open
            .iter()
            .position(|open| open.plugin == OWN_PLUGIN && open.window.id == id);
        match (index, window) {
            (Some(index), Some(window)) => {
                let open = &mut self.open[index];
                if open.window != window {
                    open.state.sync(&window);
                    open.window = window;
                    open.arrived = Instant::now();
                }
            }
            (None, Some(window)) => {
                let count = self.open.len();
                self.open(OWN_PLUGIN, window);
                // HUD windows stack in list order, so the client's goes first.
                if self.open.len() > count {
                    let own = self.open.remove(count);
                    self.open.insert(0, own);
                }
            }
            (Some(index), None) => {
                self.open.remove(index);
            }
            (None, None) => {}
        }
    }

    /// Sets the top of the HUD slot below the minimap for this frame.
    pub fn set_hud_top(&mut self, top: f32) {
        self.hud_top = top;
    }

    /// Finds the units of the anchored windows on screen this frame. A unit that `find` does not return hides its
    /// window until it comes back.
    pub fn place_units(&mut self, find: impl Fn(u32) -> Option<UnitAnchor>) {
        self.unit_anchors.clear();
        for open in &mut self.open {
            let Placement::Anchored { unit_id, .. } = open.window.placement else {
                continue;
            };
            match find(unit_id) {
                Some(anchor) => {
                    if open.waiting_for_unit {
                        tracing::info!(
                            "plugin window {}/{}: unit {unit_id} is known now",
                            open.plugin,
                            open.window.id
                        );
                        open.waiting_for_unit = false;
                    }
                    self.unit_anchors.insert(unit_id, anchor);
                }
                None if !open.waiting_for_unit => {
                    // Info, not debug: the client logs at info by default, and this fires once per wait.
                    tracing::info!(
                        "plugin window {}/{} hidden: the client does not know unit {unit_id}",
                        open.plugin,
                        open.window.id
                    );
                    open.waiting_for_unit = true;
                }
                None => {}
            }
        }
    }

    pub fn close(&mut self, plugin: &str, id: &str) {
        tracing::info!("plugin window {plugin}/{id} closed");
        self.open
            .retain(|open| !(open.plugin == plugin && open.window.id == id));
    }

    /// Plugin windows belong to the session: the server forgets them when the character leaves.
    pub fn clear(&mut self) {
        if !self.open.is_empty() {
            tracing::info!("plugin windows cleared: {} open", self.open.len());
        }
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
        let (screen_w, screen_h) = (ui.ctx.screen_width, ui.ctx.screen_height);
        let placement = self.open[index].window.placement;
        let fixed = match placement {
            Placement::Floating => None,
            Placement::Hud { .. } => {
                // Stacks under the windows of its slot that opened before it.
                let mut top = self.hud_top;
                for open in &self.open[..index] {
                    if matches!(open.window.placement, Placement::Hud { .. }) {
                        top += open.size(ui.atlas, screen_h).1 + HUD_STACK_GAP;
                    }
                }
                let x = screen_w - HUD_MARGIN - f32::from(self.open[index].window.width);
                Some((x.max(0.0), top))
            }
            Placement::Anchored { unit_id, offset_y } => {
                // Its unit is out of view: the bubble waits, shown again when the unit comes back.
                let Some(anchor) = self.unit_anchors.get(&unit_id).copied() else {
                    self.open[index].state.widget_rects.clear();
                    return Vec::new();
                };
                let (width, height) = self.open[index].size(ui.atlas, screen_h);
                Some(anchored_position(anchor, offset_y, width, height, screen_w, screen_h))
            }
        };
        let slot = self.open[index].slot;
        let (events, closed, typing) = self.open[index].build(ui, ctx, fixed);
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

    /// Height of the title bar, or of the header line of a frameless window that can collapse; 0 for a frameless
    /// window that cannot.
    fn head_h(&self, atlas: &FontAtlas) -> f32 {
        match self.window.frame {
            Frame::Standard => TITLE_H,
            Frame::None if self.window.collapsible => atlas.line_height + 2.0,
            Frame::None => 0.0,
        }
    }

    /// Padding around the content: the standard frame's, none for a frameless window.
    fn pad(&self) -> f32 {
        match self.window.frame {
            Frame::Standard => PAD,
            Frame::None => 0.0,
        }
    }

    fn size(&self, atlas: &FontAtlas, screen_h: f32) -> (f32, f32) {
        let width = f32::from(self.window.width);
        let head = self.head_h(atlas);
        let folded = self.window.collapsible && self.collapsed;
        let height = match self.window.height {
            Some(_) if folded => head,
            Some(height) => f32::from(height),
            None if folded => head,
            None => {
                let pad = self.pad();
                let content = measure(atlas, &self.window.node, width - 2.0 * pad, Axis::Column).1;
                head + 2.0 * pad + content
            }
        };
        (width, height.min(screen_h.max(TITLE_H)))
    }

    /// The events of the frame, whether the player closed the window, and whether one of its inputs has the keyboard.
    /// `fixed` is where an anchored or HUD window is drawn; a floating window is placed and moved by the player.
    fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        fixed: Option<(f32, f32)>,
    ) -> (Vec<GameEvent>, bool, bool) {
        let window_id = WidgetId(self.base());
        let (width, height) = self.size(ui.atlas, ui.ctx.screen_height);
        let head = self.head_h(ui.atlas);
        let seconds = ui.elapsed_secs - *self.shown_at.get_or_insert(ui.elapsed_secs);
        let win = match fixed {
            Some((x, y)) => ui.window_fixed(window_id, width, height, x, y),
            None => {
                if !self.placed {
                    let cascade = self.slot as f32 * CASCADE;
                    let x = ((ui.ctx.screen_width - width) / 2.0).floor() + cascade;
                    let y = ((ui.ctx.screen_height - height) / 3.0).floor() + cascade;
                    ui.set_window_position(window_id, x, y);
                    self.placed = true;
                }
                ui.window_at(window_id, width, height, head, 0.0, 0.0)
            }
        };
        ui.interact(window_id, win);

        let mut toggled = false;
        match self.window.frame {
            Frame::Standard => {
                draw_titlebar(ui, win.x, win.y, width, TITLE_H);
                ui.text(
                    win.x + 16.0,
                    win.y + TITLE_H - 3.0,
                    &self.window.title,
                    text_color(),
                );
                if !self.collapsed {
                    fallback::window_body(ui, win.x, win.y + TITLE_H, width, height - TITLE_H);
                }
                if self.window.collapsible {
                    // Left of the close button, when there is one.
                    let close_space = if self.window.closable {
                        CLOSE_BTN_SIZE + 3.0
                    } else {
                        0.0
                    };
                    let rect = Rect::new(
                        win.x + width - CLOSE_BTN_SIZE - 3.0 - close_space,
                        win.y + (TITLE_H - CLOSE_BTN_SIZE) / 2.0,
                        CLOSE_BTN_SIZE,
                        CLOSE_BTN_SIZE,
                    );
                    let response = ui.interact(WidgetId(self.base() + COLLAPSE_OFFSET), rect);
                    if response.hovered() {
                        ui.any_interactive_hovered = true;
                    }
                    let glyph = if self.collapsed { '+' } else { '-' };
                    draw_sys_button(
                        ui,
                        rect,
                        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
                        response.hovered(),
                        Some(glyph),
                    );
                    toggled = response.clicked();
                }
            }
            Frame::None if self.window.collapsible => {
                // The title as a header line, with a shadowed and outlined text and no background.
                let rect = Rect::new(win.x, win.y, width, head);
                let response = ui.interact(WidgetId(self.base() + COLLAPSE_OFFSET), rect);
                if response.hovered() {
                    ui.any_interactive_hovered = true;
                }
                let marker = if self.collapsed { '+' } else { '-' };
                let label = format!("{marker} {}", self.window.title);
                let line_y = baseline(ui.atlas, win.y, head);
                let look = TextLook {
                    bold: false,
                    align: Align::Start,
                    shadow: true,
                    outline: true,
                };
                paint_text(ui, win.x, line_y, &label, HEADER_COLOR, look);
                toggled = response.clicked();
            }
            Frame::None => {}
        }
        if toggled {
            self.collapsed = !self.collapsed;
        }

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
        let pad = self.pad();
        let elapsed = self.arrived.elapsed().as_secs() as u32;
        let collapsed = self.collapsed;
        let mut painter = Painter {
            window_id: &self.window.id,
            base: self.base(),
            seconds,
            elapsed,
            state: &mut self.state,
            events: Vec::new(),
            widget_clicked: false,
            typing: false,
        };
        let avail = width - 2.0 * pad;
        if !collapsed {
            let (_, content_h) = measure(ui.atlas, &self.window.node, avail, Axis::Column);
            let mut index = 0;
            painter.draw(
                ui,
                ctx.data,
                &self.window.node,
                &mut index,
                Rect::new(win.x + pad, win.y + head + pad, avail, content_h),
                avail,
                Axis::Column,
            );
        }

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
    /// Whole seconds since the window arrived, which the countdowns have counted down by.
    elapsed: u32,
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
                    let fills = matches!(
                        child,
                        Node::Text { .. } | Node::Countdown { .. } | Node::Separator
                    );
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
                shadow,
                outline,
            } => {
                let lines = text_lines(atlas, text, *bold, *wrap, avail);
                let look = TextLook {
                    bold: *bold,
                    align: *align,
                    shadow: *shadow,
                    outline: *outline,
                };
                paint_lines(ui, atlas, rect, &lines, color.as_deref(), look);
            }
            Node::Countdown {
                seconds,
                color,
                bold,
                align,
                shadow,
                outline,
            } => {
                let left = seconds.saturating_sub(self.elapsed);
                let look = TextLook {
                    bold: *bold,
                    align: *align,
                    shadow: *shadow,
                    outline: *outline,
                };
                paint_lines(
                    ui,
                    atlas,
                    rect,
                    &[format_countdown(left)],
                    color.as_deref(),
                    look,
                );
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
/// How a line of text is drawn over whatever is behind it.
#[derive(Clone, Copy)]
struct TextLook {
    bold: bool,
    align: Align,
    shadow: bool,
    outline: bool,
}

/// Draws the lines of a text node or countdown, one under the other inside `rect`.
fn paint_lines(
    ui: &mut UiFrame,
    atlas: &FontAtlas,
    rect: Rect,
    lines: &[String],
    color: Option<&str>,
    look: TextLook,
) {
    // Shaded text is drawn over the game, where the window body's dark default would vanish into its own outline.
    let fallback = if look.shadow || look.outline {
        HEADER_COLOR
    } else {
        text_color()
    };
    let color = color.and_then(parse_rgb).unwrap_or(fallback);
    let line_h = atlas.line_height;
    for (i, line) in lines.iter().enumerate() {
        let top = rect.y + i as f32 * line_h;
        let x = rect.x + aligned(look.align, rect.w - text_width(atlas, line, look.bold));
        paint_text(ui, x, baseline(atlas, top, line_h), line, color, look);
    }
}

/// One line of text, with its shadow or outline drawn in black underneath.
fn paint_text(
    ui: &mut UiFrame,
    x: f32,
    baseline: f32,
    line: &str,
    color: [f32; 4],
    look: TextLook,
) {
    if look.outline {
        let plain = draw::strip_color_codes(line);
        for (dx, dy) in OUTLINE_OFFSETS {
            shade(ui, x + dx, baseline + dy, &plain, look.bold);
        }
    } else if look.shadow {
        let plain = draw::strip_color_codes(line);
        shade(ui, x + 1.0, baseline + 1.0, &plain, look.bold);
    }
    if look.bold {
        ui.text_bold(x, baseline, line, color);
    } else {
        ui.colored_text(x, baseline, line, color);
    }
}

fn shade(ui: &mut UiFrame, x: f32, y: f32, plain: &str, bold: bool) {
    if bold {
        ui.text_bold(x, y, plain, SHADOW_COLOR);
    } else {
        ui.text(x, y, plain, SHADOW_COLOR);
    }
}

/// Above the unit's sprite, centred on it and kept on screen.
fn anchored_position(
    anchor: UnitAnchor,
    offset_y: i16,
    width: f32,
    height: f32,
    screen_w: f32,
    screen_h: f32,
) -> (f32, f32) {
    let x = anchor.x - width / 2.0;
    let y = anchor.top - f32::from(offset_y) - height;
    (
        x.clamp(0.0, (screen_w - width).max(0.0)),
        y.clamp(0.0, (screen_h - height).max(0.0)),
    )
}

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
        Node::Countdown { seconds, bold, .. } => (
            text_width(atlas, &format_countdown(*seconds), *bold),
            atlas.line_height,
        ),
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
    fn button(id: &str) -> Node {
        Node::Button {
            id: id.to_string(),
            label: "Go".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn an_anchored_window_sits_above_its_unit_and_waits_while_the_unit_is_out_of_view() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        let bubble = Window::new("offer", "Ann", 220, button("go")).anchored(7, 4);
        windows.open(SHOP, bubble);

        windows.place_units(|unit| (unit == 7).then_some(UnitAnchor { x: 400.0, top: 300.0 }));
        frame(&mut windows, &mut state, None, "");
        let go = windows.widget_rect(SHOP, "offer", "go").unwrap();
        assert!(go.y + go.h <= 296.0, "{go:?}");

        windows.place_units(|_| None);
        frame(&mut windows, &mut state, None, "");
        assert!(windows.widget_rect(SHOP, "offer", "go").is_none());
        assert!(!windows.is_empty(), "a hidden bubble stays open");
        assert!(windows.open[0].waiting_for_unit, "the wait is logged once");

        windows.place_units(|_| Some(UnitAnchor { x: 400.0, top: 300.0 }));
        assert!(!windows.open[0].waiting_for_unit);
    }

    #[test]
    fn anchored_windows_are_centred_above_the_unit_and_kept_on_screen() {
        let above = UnitAnchor { x: 400.0, top: 300.0 };
        assert_eq!(anchored_position(above, 4, 200.0, 100.0, 800.0, 600.0), (300.0, 196.0));
        let corner = UnitAnchor { x: 10.0, top: 20.0 };
        assert_eq!(anchored_position(corner, 0, 200.0, 100.0, 800.0, 600.0), (0.0, 0.0));
        let edge = UnitAnchor { x: 790.0, top: 595.0 };
        assert_eq!(anchored_position(edge, 0, 200.0, 100.0, 800.0, 600.0), (600.0, 495.0));
    }

    #[test]
    fn hud_windows_stack_under_the_hud_top_in_opening_order() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        let hud = |id: &str| {
            Window::new(id, "Quests", 190, button("go"))
                .hud(plugin_ui::HudSlot::BelowMinimap)
                .frameless()
        };
        windows.open(SHOP, hud("first"));
        windows.open(SHOP, hud("second"));
        windows.set_hud_top(30.0);
        frame(&mut windows, &mut state, None, "");

        let first = windows.widget_rect(SHOP, "first", "go").unwrap();
        let second = windows.widget_rect(SHOP, "second", "go").unwrap();
        assert!(first.y >= 30.0, "{first:?}");
        assert!(second.y >= first.y + first.h + HUD_STACK_GAP, "{first:?} {second:?}");
    }

    #[test]
    fn the_clients_own_hud_window_stacks_above_the_plugins_and_hides_with_none() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        let hud = |id: &str| {
            Window::new(id, "Quests", 190, button("go"))
                .hud(plugin_ui::HudSlot::BelowMinimap)
                .frameless()
                .collapsible()
        };
        windows.open(SHOP, hud("plugin"));
        windows.show_own("quest", Some(hud("quest")));
        windows.set_hud_top(30.0);
        frame(&mut windows, &mut state, None, "");
        let own = windows.widget_rect(OWN_PLUGIN, "quest", "go").unwrap();
        let plugin = windows.widget_rect(SHOP, "plugin", "go").unwrap();
        assert!(plugin.y > own.y, "{own:?} {plugin:?}");

        // Sent again every frame, the window keeps the player's folding.
        windows.open[0].collapsed = true;
        windows.show_own("quest", Some(hud("quest")));
        assert!(windows.open[0].collapsed);

        windows.show_own("quest", None);
        assert_eq!(windows.window_ids().len(), 1);
        assert!(windows.widget_rect(SHOP, "plugin", "go").is_some());
    }

    #[test]
    fn outlined_text_without_a_colour_is_light_and_plain_text_keeps_the_body_colour() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut state = StateCache::new();
        let mut ui = TestFrame::new().build(&mut ctx, &mut state);
        let atlas = ui.atlas;
        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let lines = ["Egg 0/7".to_string()];
        let look = |outline| TextLook {
            bold: false,
            align: Align::Start,
            shadow: outline,
            outline,
        };

        paint_lines(&mut ui, atlas, rect, &lines, None, look(true));
        assert_eq!(ui.draw_calls.last().unwrap().vertices[0].color, HEADER_COLOR);
        paint_lines(&mut ui, atlas, rect, &lines, None, look(false));
        assert_eq!(ui.draw_calls.last().unwrap().vertices[0].color, text_color());
        paint_lines(&mut ui, atlas, rect, &lines, Some("ffd34d"), look(true));
        assert_eq!(
            ui.draw_calls.last().unwrap().vertices[0].color,
            parse_rgb("ffd34d").unwrap()
        );
    }

    #[test]
    fn a_collapsed_window_draws_no_content_and_stays_collapsed_across_a_reopen() {
        let mut windows = PluginWindows::default();
        let mut state = StateCache::new();
        let tracker = Window::new("tracker", "Quests", 190, button("go")).collapsible();
        windows.open(SHOP, tracker.clone());
        frame(&mut windows, &mut state, None, "");
        assert!(windows.widget_rect(SHOP, "tracker", "go").is_some());

        windows.open[0].collapsed = true;
        frame(&mut windows, &mut state, None, "");
        assert!(windows.widget_rect(SHOP, "tracker", "go").is_none());

        windows.open(SHOP, tracker);
        assert!(windows.open[0].collapsed, "a re-sent window keeps its fold");
    }

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
