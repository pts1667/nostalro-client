use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
use crate::helper::window_chrome::{
    draw_footer, draw_hline, draw_sys_button, draw_textured_quad, draw_titlebar, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use plugin_ui::{HudSlot, Node, Window as PluginWindow};
use ragnarok_game::data_table::DataTable;
use ragnarok_game::event::GameEvent;
use ragnarok_game::quest::{Quest, QuestLog};
use ragnarok_ui::draw::{self, DrawCall, TextureRef, strip_color_codes, word_wrap};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const QUEST_WINDOW_ID: WidgetId = WidgetId(3700);
const CLOSE_BTN_ID: WidgetId = WidgetId(3701);
const VIEW_BTN_ID: WidgetId = WidgetId(3702);
const FOOTER_CLOSE_BTN_ID: WidgetId = WidgetId(3703);
const TRACK_BTN_ID: WidgetId = WidgetId(3707);
const TAB_BASE_ID: u32 = 3710;
const ROW_BASE_ID: u32 = 3720;
const LIST_SCROLL: ScrollbarIds = ScrollbarIds {
    up: WidgetId(3704),
    down: WidgetId(3705),
    thumb: WidgetId(3706),
};

pub const QUEST_DETAIL_WINDOW_ID: WidgetId = WidgetId(3750);
const DETAIL_CLOSE_BTN_ID: WidgetId = WidgetId(3751);
const DETAIL_TRACK_BTN_ID: WidgetId = WidgetId(3752);
const DETAIL_FOOTER_CLOSE_BTN_ID: WidgetId = WidgetId(3753);
const DETAIL_SCROLL: ScrollbarIds = ScrollbarIds {
    up: WidgetId(3754),
    down: WidgetId(3755),
    thumb: WidgetId(3756),
};

/// The id of the tracked quest's window among the client's own HUD windows.
pub const QUEST_TRACKER_ID: &str = "quest";
/// As wide as the `alive_towns` tracker it stacks with.
const TRACKER_W: u16 = 180;
const TRACKER_DONE: &str = "8cf08c";
const TRACKER_TIME: &str = "c8c8c8";

const WIN_W: f32 = 280.0;
const WIN_H: f32 = 200.0;
const TITLE_H: f32 = 17.0;
const FOOTER_H: f32 = 24.0;
const CLOSE_BTN_SIZE: f32 = 11.0;
const TAB_STRIP_W: f32 = 20.0;
const TAB_STRIP_H: f32 = 94.0;
const TAB_COUNT: usize = 3;
const ROW_H: f32 = 36.0;
const ICON: f32 = 24.0;
const BUTTON_W: f32 = 42.0;
const TRACK_BUTTON_W: f32 = 54.0;

const DETAIL_W: f32 = 320.0;
const DETAIL_PAD: f32 = 10.0;
/// Quest artwork is square, `que_noimage`'s size.
const DETAIL_IMG: f32 = 83.0;
const LINE_H: f32 = 14.0;
/// Text lines shown before the body scrolls.
const DETAIL_MAX_LINES: usize = 18;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const PANEL_BG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BORDER: [f32; 4] = [0.761, 0.761, 0.761, 1.0];
const SELECTION_COLOR: [f32; 4] = [0.451, 0.62, 0.937, 1.0];
const TAB_BAR_BG: [f32; 4] = [0.71, 0.714, 0.71, 1.0];
const TAB_INACTIVE: [f32; 4] = [0.808, 0.808, 0.808, 1.0];
const TEXT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const LABEL_COLOR: [f32; 4] = [0.14, 0.20, 0.38, 1.0];

const TAB_LABELS: [&str; TAB_COUNT] = ["On", "Off", "All"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuestTab {
    On = 0,
    Off = 1,
    All = 2,
}

impl QuestTab {
    fn from_index(i: usize) -> Self {
        match i {
            0 => Self::On,
            1 => Self::Off,
            _ => Self::All,
        }
    }

    fn matches(self, quest: &Quest) -> bool {
        match self {
            Self::On => quest.active,
            Self::Off => !quest.active,
            Self::All => true,
        }
    }
}

fn fill(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    let (v, i) = draw::quad_vertices(x, y, w, h, color);
    ui.draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
}

fn quest_title(data: &DataTable, id: u32) -> String {
    data.quest_display
        .as_ref()
        .map(|t| t.title(id))
        .unwrap_or_else(|| format!("Quest {id}"))
}

fn quest_icon(data: &DataTable, id: u32) -> Option<String> {
    data.quest_display.as_ref().map(|t| t.icon_texture(id))
}

/// The Track button's label and the event it sends for `id`: following it, or no longer.
fn track_toggle(log: &QuestLog, id: u32) -> (String, GameEvent) {
    if log.tracked == Some(id) {
        (
            rust_i18n::t!("quest.untrack").to_string(),
            GameEvent::TrackQuest { quest_id: None },
        )
    } else {
        (
            rust_i18n::t!("quest.track").to_string(),
            GameEvent::TrackQuest { quest_id: Some(id) },
        )
    }
}

/// An objective as the tracker and the detail window show it; the total is left out until the server sends it.
fn objective_progress(current: i16, required: i16) -> String {
    if required > 0 {
        format!("{current}/{required}")
    } else {
        current.to_string()
    }
}

/// The HUD window of the tracked quest, under the minimap: its objectives, or its summary when it has none, and
/// the time left on a timed quest. `now` is the Unix time.
pub fn quest_tracker(log: &QuestLog, data: &DataTable, now: u64) -> Option<PluginWindow> {
    let quest = log.tracked_quest()?;
    let mut lines = Vec::new();
    for objective in &quest.objectives {
        let done = objective.required > 0 && objective.current >= objective.required;
        let text = format!(
            "{} {}",
            objective.name,
            objective_progress(objective.current, objective.required)
        );
        let node = Node::text(text).outlined();
        lines.push(if done { node.color(TRACKER_DONE) } else { node });
    }
    if lines.is_empty() {
        let summary = data
            .quest_display
            .as_ref()
            .map(|t| t.summary(quest.id))
            .unwrap_or_default();
        if !summary.is_empty() {
            lines.push(Node::wrapped(summary).outlined());
        }
    }
    if let Some(end) = quest.end_time {
        let left = u64::from(end).saturating_sub(now);
        lines.push(
            Node::countdown(u32::try_from(left).unwrap_or(u32::MAX))
                .outlined()
                .color(TRACKER_TIME),
        );
    }
    let mut window = PluginWindow::new(
        QUEST_TRACKER_ID,
        &quest_title(data, quest.id),
        TRACKER_W,
        Node::column(lines),
    )
    .hud(HudSlot::BelowMinimap)
    .frameless()
    .collapsible();
    window.closable = false;
    Some(window)
}

pub struct QuestWindow {
    open: bool,
    tab: QuestTab,
    scroll: usize,
    selected: Option<u32>,
    pending_toggle: Option<(u32, bool)>,
}

impl Default for QuestWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            tab: QuestTab::On,
            scroll: 0,
            selected: None,
            pending_toggle: None,
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

    fn tab_strip(&mut self, ui: &mut UiFrame, x: f32, y: f32) {
        fill(ui, x, y, TAB_STRIP_W, TAB_STRIP_H, TAB_BAR_BG);
        let tab_h = TAB_STRIP_H / TAB_COUNT as f32;
        for i in 0..TAB_COUNT {
            let ty = y + i as f32 * tab_h;
            let rect = Rect::new(x, ty, TAB_STRIP_W, tab_h);
            let selected = self.tab as usize == i;
            fill(
                ui,
                x + 1.0,
                ty + 1.0,
                TAB_STRIP_W - 2.0,
                tab_h - 2.0,
                if selected { WHITE } else { TAB_INACTIVE },
            );
            ui.text(x + 2.0, ty + tab_h / 2.0 + 4.0, TAB_LABELS[i], TEXT);
            let resp = ui.interact(WidgetId(TAB_BASE_ID + i as u32), rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() && !selected {
                self.tab = QuestTab::from_index(i);
                self.scroll = 0;
            }
        }
    }

    fn list_panel(
        &mut self,
        ui: &mut UiFrame,
        quest_log: &QuestLog,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        data: &DataTable,
    ) {
        fill(ui, x, y, w, h, PANEL_BG);

        let ids: Vec<u32> = quest_log
            .quests
            .iter()
            .filter(|q| self.tab.matches(q))
            .map(|q| q.id)
            .collect();

        let visible = (h / ROW_H).max(1.0) as usize;
        let max_scroll = ids.len().saturating_sub(visible);
        let has_bar = max_scroll > 0;
        let bar_w = if has_bar { SCROLLBAR_W } else { 0.0 };
        if has_bar {
            let content_rect = Rect::new(x, y, w, h);
            self.scroll = scrollbar::scrollbar(
                ui,
                LIST_SCROLL,
                self.scroll,
                visible,
                max_scroll,
                content_rect,
                x + w - SCROLLBAR_W,
                y,
                h,
            );
        } else {
            self.scroll = 0;
        }

        for row in 0..visible {
            let Some(&id) = ids.get(self.scroll + row) else {
                break;
            };
            let row_y = y + row as f32 * ROW_H;
            let row_rect = Rect::new(x, row_y, w - bar_w, ROW_H);
            if self.selected == Some(id) {
                fill(
                    ui,
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    SELECTION_COLOR,
                );
            }
            fill(ui, x, row_y + ROW_H - 1.0, w - bar_w, 1.0, BORDER);

            if let Some(icon) = quest_icon(data, id) {
                let icon_y = row_y + (ROW_H - ICON) / 2.0;
                draw_textured_quad(ui, x + 4.0, icon_y, ICON, ICON, &icon);
            }

            let baseline = row_y + ROW_H / 2.0 + 4.0;
            ui.text(x + 4.0 + ICON + 6.0, baseline, &quest_title(data, id), TEXT);
            if quest_log.tracked == Some(id) {
                let label = rust_i18n::t!("quest.tracking");
                let label_w = ui.atlas.measure_text(&label);
                ui.text(x + w - bar_w - label_w - 6.0, baseline, &label, LABEL_COLOR);
            }

            let resp = ui.interact(WidgetId(ROW_BASE_ID + row as u32), row_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                self.selected = Some(id);
            }
            if resp.right_clicked() {
                self.selected = Some(id);
                let active = quest_log
                    .quests
                    .iter()
                    .find(|q| q.id == id)
                    .map(|q| q.active)
                    .unwrap_or(false);
                self.pending_toggle = Some((id, !active));
            }
        }
    }
}

impl Window for QuestWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for QuestWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let data = ctx.data;
        let quest_log = ctx.quest_log;
        if !self.open {
            return Vec::new();
        }
        if let Some(sel) = self.selected
            && !quest_log.quests.iter().any(|q| q.id == sel)
        {
            self.selected = None;
        }
        let mut events = Vec::new();
        let tc = text_color();
        self.pending_toggle = None;

        let win = ui.window_at(QUEST_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 0.0, 120.0);
        let (x, y) = (win.x, win.y);
        ui.interact(QUEST_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + TITLE_H - 3.0, &rust_i18n::t!("quest.quest_list"), tc);

        if title_close_button(ui, CLOSE_BTN_ID, x, y, WIN_W) {
            self.open = false;
            return events;
        }

        let body_y = y + TITLE_H;
        let body_h = WIN_H - TITLE_H - FOOTER_H;
        self.tab_strip(ui, x, body_y);
        let list_x = x + TAB_STRIP_W;
        let list_w = WIN_W - TAB_STRIP_W;
        self.list_panel(ui, quest_log, list_x, body_y, list_w, body_h, data);
        fill(ui, x + TAB_STRIP_W, body_y, 1.0, body_h, BORDER);

        let footer_y = y + WIN_H - FOOTER_H;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        let view_rect = Rect::new(x + 6.0, footer_y + 2.0, BUTTON_W, 20.0);
        if ui
            .button(VIEW_BTN_ID, view_rect, &rust_i18n::t!("quest.view"))
            .clicked()
        {
            if let Some(id) = self.selected {
                events.push(GameEvent::OpenQuestDetail { quest_id: id });
            }
        }
        if let Some(id) = self.selected {
            let (label, event) = track_toggle(quest_log, id);
            let track_rect = Rect::new(x + 12.0 + BUTTON_W, footer_y + 2.0, TRACK_BUTTON_W, 20.0);
            if ui.button(TRACK_BTN_ID, track_rect, &label).clicked() {
                events.push(event);
            }
        }
        let close2_rect = Rect::new(x + WIN_W - BUTTON_W - 6.0, footer_y + 2.0, BUTTON_W, 20.0);
        if ui
            .button(FOOTER_CLOSE_BTN_ID, close2_rect, &rust_i18n::t!("common.close"))
            .clicked()
        {
            self.open = false;
            return events;
        }

        if let Some((quest_id, active)) = self.pending_toggle.take() {
            events.push(GameEvent::RequestToggleQuestActive { quest_id, active });
        }

        events
    }
}

/// The close button of a title bar `w` wide at (`x`, `y`); whether it was clicked.
fn title_close_button(ui: &mut UiFrame, id: WidgetId, x: f32, y: f32, w: f32) -> bool {
    let rect = Rect::new(
        x + w - CLOSE_BTN_SIZE - 3.0,
        y + (TITLE_H - CLOSE_BTN_SIZE) / 2.0,
        CLOSE_BTN_SIZE,
        CLOSE_BTN_SIZE,
    );
    let resp = ui.interact(id, rect);
    if resp.hovered() {
        ui.any_interactive_hovered = true;
    }
    draw_sys_button(
        ui,
        rect,
        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
        resp.hovered(),
        Some('x'),
    );
    resp.clicked()
}

/// One line of the detail window's text, with an objective's progress at its right.
struct DetailLine {
    text: String,
    progress: Option<String>,
    color: [f32; 4],
    bold: bool,
}

impl DetailLine {
    fn plain(text: String, color: [f32; 4], bold: bool) -> Self {
        Self {
            text,
            progress: None,
            color,
            bold,
        }
    }
}

pub struct QuestDetailWindow {
    open: bool,
    quest_id: Option<u32>,
    /// The summary and description wrapped to the window, for `wrapped_source`.
    wrapped_summary: Vec<String>,
    wrapped_desc: Vec<String>,
    wrapped_source: u32,
    scroll: usize,
}

impl Default for QuestDetailWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestDetailWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            quest_id: None,
            wrapped_summary: Vec::new(),
            wrapped_desc: Vec::new(),
            wrapped_source: u32::MAX,
            scroll: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn quest_id(&self) -> Option<u32> {
        self.quest_id
    }

    pub fn open(&mut self, quest_id: u32) {
        self.quest_id = Some(quest_id);
        self.open = true;
        self.wrapped_summary.clear();
        self.wrapped_desc.clear();
        self.wrapped_source = u32::MAX;
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn wrap_text(&mut self, ui: &UiFrame, data: &DataTable, id: u32) {
        if self.wrapped_source == id {
            return;
        }
        let table = data.quest_display.as_ref();
        let width = DETAIL_W - 2.0 * DETAIL_PAD - SCROLLBAR_W;
        let wrap = |text: String| -> Vec<String> {
            if text.is_empty() {
                return Vec::new();
            }
            word_wrap(
                &text,
                width,
                |t| ui.atlas.measure_text(&strip_color_codes(t)),
                false,
            )
        };
        self.wrapped_summary = wrap(table.map(|t| t.summary(id)).unwrap_or_default());
        self.wrapped_desc = wrap(table.map(|t| t.description(id)).unwrap_or_default());
        self.wrapped_source = id;
    }

    /// The summary, the description and the objectives, a blank line between each part there is.
    fn lines(&self, quest: &Quest) -> Vec<DetailLine> {
        let mut lines = Vec::new();
        let part = |lines: &mut Vec<DetailLine>, section: Vec<DetailLine>| {
            if section.is_empty() {
                return;
            }
            if !lines.is_empty() {
                lines.push(DetailLine::plain(String::new(), TEXT, false));
            }
            lines.extend(section);
        };
        let summary = self
            .wrapped_summary
            .iter()
            .map(|line| DetailLine::plain(line.clone(), LABEL_COLOR, true))
            .collect();
        part(&mut lines, summary);
        let description = self
            .wrapped_desc
            .iter()
            .map(|line| DetailLine::plain(line.clone(), TEXT, false))
            .collect();
        part(&mut lines, description);
        if !quest.objectives.is_empty() {
            let mut objectives = vec![DetailLine::plain(
                rust_i18n::t!("quest.objectives").to_string(),
                LABEL_COLOR,
                true,
            )];
            objectives.extend(quest.objectives.iter().map(|objective| DetailLine {
                text: objective.name.clone(),
                progress: Some(objective_progress(objective.current, objective.required)),
                color: TEXT,
                bold: false,
            }));
            part(&mut lines, objectives);
        }
        lines
    }
}

impl Window for QuestDetailWindow {
    fn window_size(&self) -> (f32, f32) {
        (DETAIL_W, TITLE_H + FOOTER_H + DETAIL_IMG + 2.0 * DETAIL_PAD)
    }
}

impl InGameWindow for QuestDetailWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let data = ctx.data;
        if !self.open {
            return Vec::new();
        }
        let Some(quest) = self.quest_id.and_then(|id| ctx.quest_log.get(id)) else {
            self.open = false;
            return Vec::new();
        };
        let mut events = Vec::new();
        let id = quest.id;
        self.wrap_text(ui, data, id);
        let lines = self.lines(quest);

        // The artwork when the quest has some, else its icon.
        let image = data.quest_display.as_ref().and_then(|t| t.image_texture(id));
        let (art, art_size) = match image {
            Some(path) => (Some(path), DETAIL_IMG),
            None => (quest_icon(data, id), ICON),
        };
        let head_lines = if quest.end_time.is_some() { 2.0 } else { 1.0 };
        let head_h = art_size.max(head_lines * LINE_H);
        let shown = lines.len().min(DETAIL_MAX_LINES);
        let text_h = shown as f32 * LINE_H;
        let body_h = 2.0 * DETAIL_PAD + head_h + if shown > 0 { 8.0 + text_h } else { 0.0 };
        let win_h = TITLE_H + body_h + FOOTER_H;

        let win = ui.window_at(QUEST_DETAIL_WINDOW_ID, DETAIL_W, win_h, TITLE_H, 370.0, 60.0);
        let (x, y) = (win.x, win.y);
        ui.interact(QUEST_DETAIL_WINDOW_ID, Rect::new(x, y, DETAIL_W, win_h));

        draw_titlebar(ui, x, y, DETAIL_W, TITLE_H);
        ui.text(x + 16.0, y + TITLE_H - 3.0, &rust_i18n::t!("quest.details"), text_color());
        if title_close_button(ui, DETAIL_CLOSE_BTN_ID, x, y, DETAIL_W) {
            self.open = false;
            return events;
        }

        let body_y = y + TITLE_H;
        fill(ui, x, body_y, DETAIL_W, body_h, PANEL_BG);

        let head_y = body_y + DETAIL_PAD;
        let mut title_x = x + DETAIL_PAD;
        if let Some(path) = &art {
            draw_textured_quad(ui, title_x, head_y + (head_h - art_size) / 2.0, art_size, art_size, path);
            title_x += art_size + 8.0;
        }
        let first_top = head_y + (head_h - head_lines * LINE_H) / 2.0;
        ui.text_bold(title_x, first_top + LINE_H - 3.0, &quest_title(data, id), TEXT);
        if let Some(end) = quest.end_time {
            ui.text(title_x, first_top + 2.0 * LINE_H - 3.0, &format_end_time(end), LABEL_COLOR);
        }

        if shown > 0 {
            let rule_y = head_y + head_h + 4.0;
            draw_hline(ui, x + DETAIL_PAD, rule_y, DETAIL_W - 2.0 * DETAIL_PAD);
            let text_top = rule_y + 4.0;
            let max_scroll = lines.len() - shown;
            if max_scroll > 0 {
                self.scroll = scrollbar::scrollbar(
                    ui,
                    DETAIL_SCROLL,
                    self.scroll,
                    shown,
                    max_scroll,
                    Rect::new(x, text_top, DETAIL_W, text_h),
                    x + DETAIL_W - SCROLLBAR_W - 3.0,
                    text_top,
                    text_h,
                );
            } else {
                self.scroll = 0;
            }
            let right = x + DETAIL_W - DETAIL_PAD - SCROLLBAR_W;
            for (row, line) in lines.iter().skip(self.scroll).take(shown).enumerate() {
                let baseline = text_top + row as f32 * LINE_H + LINE_H - 3.0;
                if line.bold {
                    ui.text_bold(x + DETAIL_PAD, baseline, &strip_color_codes(&line.text), line.color);
                } else {
                    ui.colored_text(x + DETAIL_PAD, baseline, &line.text, line.color);
                }
                if let Some(progress) = &line.progress {
                    let w = ui.atlas.measure_text(progress);
                    ui.text(right - w, baseline, progress, line.color);
                }
            }
        }

        let footer_y = body_y + body_h;
        draw_footer(ui, x, footer_y, DETAIL_W, FOOTER_H);
        let (label, event) = track_toggle(ctx.quest_log, id);
        let track_rect = Rect::new(x + 6.0, footer_y + 2.0, TRACK_BUTTON_W, 20.0);
        if ui.button(DETAIL_TRACK_BTN_ID, track_rect, &label).clicked() {
            events.push(event);
        }
        let close_rect = Rect::new(x + DETAIL_W - BUTTON_W - 6.0, footer_y + 2.0, BUTTON_W, 20.0);
        if ui
            .button(DETAIL_FOOTER_CLOSE_BTN_ID, close_rect, &rust_i18n::t!("common.close"))
            .clicked()
        {
            self.open = false;
        }

        events
    }
}

fn format_end_time(end: u32) -> String {
    let secs = end as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("~ {year:04}/{month:02}/{day:02} {hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_formats::lua_table::QuestDisplay;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;
    use ragnarok_game::data_table::quest_display_table::QuestDisplayTable;
    use ragnarok_game::quest::{QuestListEntry, QuestObjective};
    use std::collections::HashMap;

    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    const MERCHANT: u32 = 1009;

    fn merchant_log() -> QuestLog {
        let mut log = QuestLog::default();
        log.set_list_entry(QuestListEntry {
            id: MERCHANT,
            active: true,
        });
        log
    }

    fn merchant_data(image_name: &str) -> DataTable {
        let quest = QuestDisplay {
            title: "Job Change to Merchant".into(),
            icon_name: "SG_FEEL".into(),
            image_name: image_name.into(),
            description: "Get the delivery package from the storehouse, then go to the former Swordman's \
                          Association in Prontera. When you get there, visit the Kafra Employee stationed \
                          there. Her name is Blossom."
                .into(),
            summary: "Deliver package to Prontera.".into(),
        };
        let mut data = DataTable::new();
        data.quest_display = Some(QuestDisplayTable::from_entries(HashMap::from([(
            MERCHANT, quest,
        )])));
        data
    }

    fn textures(calls: &[DrawCall]) -> Vec<String> {
        calls
            .iter()
            .filter_map(|call| match &call.texture {
                TextureRef::Named(path) => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    /// One frame of the detail window on `MERCHANT`, the mouse at (`mouse_x`, `mouse_y`) and clicking.
    fn detail_frame(
        log: &QuestLog,
        data: &DataTable,
        click: Option<(f32, f32)>,
    ) -> (Vec<GameEvent>, Vec<DrawCall>) {
        let mut win = QuestDetailWindow::new();
        win.open(MERCHANT);
        let mut character = Character::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        if let Some((x, y)) = click {
            ctx.mouse_x = x;
            ctx.mouse_y = y;
            ctx.mouse_clicked = true;
        }
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut build_ctx = crate::BuildCtx::test(&mut character, data);
        build_ctx.quest_log = log;
        let events = win.build(&mut ui, &mut build_ctx);
        (events, std::mem::take(&mut ui.draw_calls))
    }

    #[test]
    fn right_click_row_requests_active_toggle() {
        let mut log = QuestLog::default();
        log.set_list_entry(QuestListEntry {
            id: 500,
            active: true,
        });
        log.set_mission(ragnarok_game::quest::QuestMissionData {
            id: 500,
            end_time: None,
            objectives: vec![QuestObjective {
                mob_id: 1002,
                name: "Poring".into(),
                current: 1,
                required: 5,
            }],
        });

        let mut win = QuestWindow::new();
        win.toggle();

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        // First row, inside the list panel (below the 17px titlebar).
        ctx.mouse_x = TAB_STRIP_W + 40.0;
        ctx.mouse_y = 120.0 + TITLE_H + ROW_H / 2.0;
        ctx.mouse_right_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut build_ctx = crate::BuildCtx::test(&mut character, &data);
        build_ctx.quest_log = &log;
        let events = win.build(&mut ui, &mut build_ctx);

        assert!(
            events.iter().any(|e| matches!(
                e,
                GameEvent::RequestToggleQuestActive {
                    quest_id: 500,
                    active: false
                }
            )),
            "expected toggle-active event, got {events:?}"
        );
    }

    #[test]
    fn the_list_draws_each_quest_icon() {
        let log = merchant_log();
        let data = merchant_data("QUE_NOIMAGE");
        let mut win = QuestWindow::new();
        win.toggle();
        let mut character = Character::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut build_ctx = crate::BuildCtx::test(&mut character, &data);
        build_ctx.quest_log = &log;
        win.build(&mut ui, &mut build_ctx);

        assert_eq!(
            textures(&ui.draw_calls),
            vec![ragnarok_resources::ui::item::icon("sg_feel")]
        );
    }

    #[test]
    fn a_quest_without_artwork_shows_its_icon_in_place_of_an_empty_image() {
        let log = merchant_log();
        let (_, calls) = detail_frame(&log, &merchant_data("QUE_NOIMAGE"), None);
        assert_eq!(
            textures(&calls),
            vec![ragnarok_resources::ui::item::icon("sg_feel")]
        );

        let (_, calls) = detail_frame(&log, &merchant_data("que_whale"), None);
        assert_eq!(
            textures(&calls),
            vec![ragnarok_resources::ui::item::icon("que_whale")]
        );
    }

    #[test]
    fn the_long_description_wraps_inside_the_window() {
        let log = merchant_log();
        let mut win = QuestDetailWindow::new();
        win.open(MERCHANT);
        let data = merchant_data("QUE_NOIMAGE");
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        let ui = test_frame(&mut ctx, &mut state);
        win.wrap_text(&ui, &data, MERCHANT);

        assert_eq!(win.wrapped_summary, vec!["Deliver package to Prontera."]);
        assert!(win.wrapped_desc.len() > 1);
        let width = DETAIL_W - 2.0 * DETAIL_PAD - SCROLLBAR_W;
        assert!(win.wrapped_desc.iter().all(|line| ui.atlas.measure_text(line) <= width));
        let lines = win.lines(log.get(MERCHANT).unwrap());
        assert_eq!(lines.len(), 1 + 1 + win.wrapped_desc.len());
    }

    #[test]
    fn the_track_button_follows_the_quest_then_stops() {
        let mut log = merchant_log();
        let data = merchant_data("QUE_NOIMAGE");
        // The Track button sits at the left of the footer, the last 24 pixels of the window.
        let (_, calls) = detail_frame(&log, &data, None);
        let bottom = calls
            .iter()
            .flat_map(|call| call.vertices.iter().map(|v| v.position[1]))
            .fold(f32::MIN, f32::max);
        let track = (370.0 + 20.0, bottom - FOOTER_H / 2.0);

        let (events, _) = detail_frame(&log, &data, Some(track));
        assert!(matches!(
            events[..],
            [GameEvent::TrackQuest {
                quest_id: Some(MERCHANT)
            }]
        ));

        log.tracked = Some(MERCHANT);
        let (events, _) = detail_frame(&log, &data, Some(track));
        assert!(matches!(
            events[..],
            [GameEvent::TrackQuest { quest_id: None }]
        ));
    }

    #[test]
    fn the_tracker_shows_objectives_or_else_the_summary_and_the_time_left() {
        let data = merchant_data("QUE_NOIMAGE");
        let mut log = merchant_log();
        assert!(quest_tracker(&log, &data, 0).is_none());

        log.tracked = Some(MERCHANT);
        let window = quest_tracker(&log, &data, 0).expect("tracked");
        assert_eq!(window.title, "Job Change to Merchant");
        assert!(matches!(
            window.placement,
            plugin_ui::Placement::Hud { .. }
        ));
        assert!(matches!(
            window.nodes()[..],
            [Node::Column(_), Node::Text { text, .. }] if text == "Deliver package to Prontera."
        ));

        log.set_mission(ragnarok_game::quest::QuestMissionData {
            id: MERCHANT,
            end_time: Some(1_000),
            objectives: vec![
                QuestObjective {
                    mob_id: 1002,
                    name: "Poring".into(),
                    current: 10,
                    required: 10,
                },
                QuestObjective {
                    mob_id: 1063,
                    name: "Lunatic".into(),
                    current: 1,
                    required: 5,
                },
            ],
        });
        let window = quest_tracker(&log, &data, 400).expect("tracked");
        match &window.nodes()[..] {
            [
                Node::Column(_),
                Node::Text {
                    text: poring,
                    color: done,
                    ..
                },
                Node::Text { text: lunatic, color: going, .. },
                Node::Countdown { seconds, .. },
            ] => {
                assert_eq!(poring, "Poring 10/10");
                assert_eq!(done.as_deref(), Some(TRACKER_DONE));
                assert_eq!(lunatic, "Lunatic 1/5");
                assert_eq!(*going, None);
                assert_eq!(*seconds, 600);
            }
            nodes => panic!("unexpected tracker {nodes:?}"),
        }
    }
}
