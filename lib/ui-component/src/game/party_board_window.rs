//! The party board: the ads of players looking for party members, a search by
//! place or job, and a form to put up, change or take down the character's ad.
//!
//! The window does no networking. It asks with
//! [`GameEvent::RequestPartyBooking`] and the client hands what the server
//! answers, and the ads going up and down, to [`PartyBoardWindow::deliver`].

use std::time::{SystemTime, UNIX_EPOCH};

use crate::game::reference_window::{clipped_text, quad};
use crate::helper::dropdown::{Dropdown, OPTION_H};
use crate::helper::window_chrome::{
    draw_container, draw_footer, draw_sys_button, draw_titlebar, label_color, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_game::party_booking::{
    BOOKING_JOBS, BookingAd, BookingFilter, BookingNotice, BookingRequest, NO_JOB, PLACES,
    RESULTS_PER_PAGE, WANTED_JOBS, age_label, job_label, job_slots, place_name,
};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::theme::FallbackPalette as P;

pub const PARTY_BOARD_WINDOW_ID: WidgetId = WidgetId(6500);
const CLOSE_BTN_ID: WidgetId = WidgetId(6501);
const PLACE_FILTER_ID: WidgetId = WidgetId(6502);
const JOB_FILTER_ID: WidgetId = WidgetId(6503);
const NEAR_LEVEL_ID: WidgetId = WidgetId(6504);
const SEARCH_BTN_ID: WidgetId = WidgetId(6505);
const PREV_BTN_ID: WidgetId = WidgetId(6506);
const NEXT_BTN_ID: WidgetId = WidgetId(6507);
const WHISPER_BTN_ID: WidgetId = WidgetId(6508);
const MY_AD_BTN_ID: WidgetId = WidgetId(6509);
const POST_BTN_ID: WidgetId = WidgetId(6510);
const REMOVE_BTN_ID: WidgetId = WidgetId(6511);
const FORM_PLACE_ID: WidgetId = WidgetId(6512);
const BACK_BTN_ID: WidgetId = WidgetId(6513);
const CLEAR_JOBS_ID: WidgetId = WidgetId(6514);
const NEAR_LEVEL_LABEL_ID: WidgetId = WidgetId(6515);
/// One per row of results.
const ROW_BASE_ID: u32 = 6520;
/// One per job of the form, up to 30.
const CHIP_BASE_ID: u32 = 6530;
/// One per choice of the open picker, up to 40.
const PICK_BASE_ID: u32 = 6560;

const WIN_W: f32 = 480.0;
const TITLE_H: f32 = 17.0;
const TOOL_H: f32 = 28.0;
const HEAD_H: f32 = 18.0;
const ROW_H: f32 = 18.0;
const BODY_H: f32 = HEAD_H + ROW_H * RESULTS_PER_PAGE as f32;
const WELL_GAP: f32 = 5.0;
const FOOTER_H: f32 = 28.0;
const WIN_H: f32 = TITLE_H + TOOL_H + BODY_H + WELL_GAP + FOOTER_H;
const PAD: f32 = 6.0;
const BTN_H: f32 = 20.0;
const BTN_W: f32 = 60.0;
const NAV_W: f32 = 22.0;
const SYS_BTN: f32 = 11.0;
const BOX_H: f32 = 18.0;
const BASELINE: f32 = 13.0;
const CHIP_COLS: usize = 4;
const CHIP_H: f32 = 18.0;
const CHIP_GAP: f32 = 4.0;
const PICK_COLS: usize = 3;
const PICK_COL_W: f32 = 132.0;

/// Left edges of the columns of the results, from the left of the well; the
/// last entry is the right edge.
const COLUMNS: [f32; 6] = [0.0, 118.0, 152.0, 284.0, 420.0, 468.0];

const MUTED_COLOR: [f32; 4] = [0.45, 0.47, 0.55, 1.0];
const ERROR_COLOR: [f32; 4] = [0.75, 0.12, 0.12, 1.0];
const LINK_COLOR: [f32; 4] = [0.10, 0.25, 0.75, 1.0];
const HEAD_FILL: [f32; 4] = [0.82, 0.88, 0.97, 1.0];
const STRIPE: [f32; 4] = [0.95, 0.96, 0.99, 1.0];
const RULE: [f32; 4] = [0.85, 0.87, 0.92, 1.0];
const HOVER_ROW: [f32; 4] = [0.78, 0.86, 1.0, 0.7];
const SELECTED_ROW: [f32; 4] = [0.62, 0.75, 0.97, 0.8];
const OWN_ROW: [f32; 4] = [1.0, 0.93, 0.70, 1.0];
const CHIP_ON: [f32; 4] = [0.55, 0.70, 0.95, 1.0];
const CHIP_OFF: [f32; 4] = [0.93, 0.95, 0.99, 1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Board,
    MyAd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picker {
    Place,
    Job,
    FormPlace,
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Info(String),
    Error(String),
}

pub struct PartyBoardWindow {
    open: bool,
    view: View,
    place: i16,
    job: i16,
    near_level: bool,
    ads: Vec<BookingAd>,
    more: bool,
    /// Waiting for the answer to a search.
    searching: bool,
    search_next_frame: bool,
    /// Where each page after the first starts, so "<" can go back.
    pages: Vec<u32>,
    selected: Option<u32>,
    /// The character's ad, once the server has shown it.
    my_ad: Option<BookingAd>,
    /// The server refused an ad because the character has one up that the
    /// window never saw, from before a relog.
    unseen_ad: bool,
    form_place: i16,
    form_jobs: Vec<i16>,
    /// Asked after the old ad comes down, when an ad moves to another place.
    repost: Option<BookingRequest>,
    /// Requests that answers to earlier ones call for, sent on the next frame.
    outbox: Vec<GameEvent>,
    picker: Option<Picker>,
    /// Where the picker boxes were drawn last frame.
    boxes: Vec<(Picker, Rect)>,
    place_box: Dropdown,
    job_box: Dropdown,
    form_box: Dropdown,
    status: Option<Status>,
    me: String,
    my_level: i16,
}

impl Default for PartyBoardWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl PartyBoardWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            view: View::Board,
            place: 0,
            job: NO_JOB,
            near_level: false,
            ads: Vec::new(),
            more: false,
            searching: false,
            search_next_frame: false,
            pages: Vec::new(),
            selected: None,
            my_ad: None,
            unseen_ad: false,
            form_place: 0,
            form_jobs: Vec::new(),
            repost: None,
            outbox: Vec::new(),
            picker: None,
            boxes: Vec::new(),
            place_box: Dropdown::default(),
            job_box: Dropdown::default(),
            form_box: Dropdown::default(),
            status: None,
            me: String::new(),
            my_level: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the board on a fresh first page, or closes it.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.close_picker();
        if self.open {
            self.view = View::Board;
            self.status = None;
            self.new_search();
        }
    }

    /// Hands over what the server said.
    pub fn deliver(&mut self, notice: BookingNotice) {
        match notice {
            BookingNotice::Results { ads, more } => {
                self.searching = false;
                self.ads = ads;
                self.more = more;
                if self.selected.is_some_and(|index| !self.ads.iter().any(|ad| ad.index == index)) {
                    self.selected = None;
                }
                if self.my_ad.is_none() {
                    let mine = self.ads.iter().find(|ad| self.is_mine(ad)).cloned();
                    if mine.is_some() {
                        self.my_ad = mine;
                        self.unseen_ad = false;
                    }
                }
            }
            BookingNotice::Registered { ok: true } => {
                self.unseen_ad = false;
                self.status = Some(Status::Info(rust_i18n::t!("party_board.posted").into_owned()));
            }
            BookingNotice::Registered { ok: false } => {
                if self.my_ad.is_none() {
                    self.unseen_ad = true;
                }
                self.status = Some(Status::Error(rust_i18n::t!("party_board.already_posted").into_owned()));
            }
            BookingNotice::Deleted { ok } => {
                self.my_ad = None;
                self.unseen_ad = false;
                if let Some(request) = self.repost.take() {
                    self.outbox.push(GameEvent::RequestPartyBooking(request));
                } else if ok {
                    self.status = Some(Status::Info(rust_i18n::t!("party_board.removed").into_owned()));
                } else {
                    self.status = Some(Status::Error(rust_i18n::t!("party_board.nothing_to_remove").into_owned()));
                }
            }
            BookingNotice::Inserted(ad) => {
                if self.is_mine(&ad) {
                    self.my_ad = Some(ad.clone());
                    self.unseen_ad = false;
                }
                let fresh = !self.ads.iter().any(|shown| shown.index == ad.index);
                if fresh && self.pages.is_empty() && ad.matches(&self.filter()) {
                    if self.ads.len() < RESULTS_PER_PAGE {
                        self.ads.push(ad);
                    } else {
                        self.more = true;
                    }
                }
            }
            BookingNotice::Updated { index, jobs } => {
                let now = unix_now() as u32;
                for ad in self.ads.iter_mut().chain(self.my_ad.as_mut()) {
                    if ad.index == index {
                        ad.jobs = jobs;
                        ad.posted = now;
                    }
                }
                if self.my_ad.as_ref().is_some_and(|ad| ad.index == index) {
                    self.status = Some(Status::Info(rust_i18n::t!("party_board.saved").into_owned()));
                }
            }
            BookingNotice::Removed { index } => {
                self.ads.retain(|ad| ad.index != index);
                if self.my_ad.as_ref().is_some_and(|ad| ad.index == index) {
                    self.my_ad = None;
                }
                if self.selected == Some(index) {
                    self.selected = None;
                }
            }
        }
    }

    fn is_mine(&self, ad: &BookingAd) -> bool {
        !self.me.is_empty() && ad.leader == self.me
    }

    fn filter(&self) -> BookingFilter {
        BookingFilter {
            level: if self.near_level { self.my_level } else { 0 },
            place: self.place,
            job: self.job,
        }
    }

    /// Asks for the first page again on the next frame.
    fn new_search(&mut self) {
        self.pages.clear();
        self.search_next_frame = true;
    }

    fn close_picker(&mut self) {
        self.picker = None;
        self.place_box.open = false;
        self.job_box.open = false;
        self.form_box.open = false;
    }

    fn open_form(&mut self) {
        self.view = View::MyAd;
        self.status = None;
        self.close_picker();
        if let Some(ad) = &self.my_ad {
            self.form_place = ad.place;
            self.form_jobs = ad.wanted().collect();
        }
    }

    fn toggle_job(&mut self, job: i16) {
        if let Some(at) = self.form_jobs.iter().position(|j| *j == job) {
            self.form_jobs.remove(at);
        } else if self.form_jobs.len() < BOOKING_JOBS {
            self.form_jobs.push(job);
            self.status = None;
        } else {
            self.status = Some(Status::Error(
                rust_i18n::t!("party_board.too_many_jobs", count = BOOKING_JOBS).into_owned(),
            ));
        }
    }

    /// The box of a picker; opening it closes the other ones.
    fn picker_box(&mut self, ui: &mut UiFrame, which: Picker, rect: Rect, label: &str) {
        let id = match which {
            Picker::Place => PLACE_FILTER_ID,
            Picker::Job => JOB_FILTER_ID,
            Picker::FormPlace => FORM_PLACE_ID,
        };
        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        let dropdown = self.picker_box_of(which);
        let resp = dropdown.show(ui, id, rect, label, 0, screen, false);
        if resp.toggled {
            let now_open = dropdown.open;
            self.close_picker();
            if now_open {
                self.picker_box_of(which).open = true;
                self.picker = Some(which);
            }
        }
    }

    fn picker_box_of(&mut self, which: Picker) -> &mut Dropdown {
        match which {
            Picker::Place => &mut self.place_box,
            Picker::Job => &mut self.job_box,
            Picker::FormPlace => &mut self.form_box,
        }
    }

    fn draw_toolbar(&mut self, ui: &mut UiFrame, x: f32, y: f32, boxes: &mut Vec<(Picker, Rect)>) {
        let row_y = y + (TOOL_H - BOX_H) / 2.0;
        let baseline = row_y + BASELINE;
        let mut bx = x + PAD;

        let place_label = rust_i18n::t!("party_board.place");
        ui.text(bx, baseline, &place_label, label_color());
        bx += ui.atlas.measure_text(&place_label) + 4.0;
        let place_rect = Rect::new(bx, row_y, 130.0, BOX_H);
        self.picker_box(ui, Picker::Place, place_rect, &place_name(self.place));
        boxes.push((Picker::Place, place_rect));
        bx += place_rect.w + 10.0;

        let job_text = rust_i18n::t!("party_board.job");
        ui.text(bx, baseline, &job_text, label_color());
        bx += ui.atlas.measure_text(&job_text) + 4.0;
        let job_rect = Rect::new(bx, row_y, 100.0, BOX_H);
        let job_name = if self.job == NO_JOB { rust_i18n::t!("party_board.any_job").into_owned() } else { job_label(self.job) };
        self.picker_box(ui, Picker::Job, job_rect, &job_name);
        boxes.push((Picker::Job, job_rect));
        bx += job_rect.w + 10.0;

        let check = Rect::new(bx, row_y + 3.0, 12.0, 12.0);
        let mut near = self.near_level;
        let check_resp = ui.checkbox(NEAR_LEVEL_ID, check, &mut near);
        let near_text = rust_i18n::t!("party_board.near_level");
        let text_w = ui.atlas.measure_text(&near_text);
        let label_rect = Rect::new(check.x + 15.0, row_y, text_w, BOX_H);
        let label_resp = ui.interact(NEAR_LEVEL_LABEL_ID, label_rect);
        if label_resp.clicked() {
            near = !near;
        }
        ui.text(label_rect.x, baseline, &near_text, text_color());
        if check_resp.hovered() || label_resp.hovered() {
            ui.any_interactive_hovered = true;
            let tip = rust_i18n::t!("party_board.near_level_tip", span = ragnarok_game::party_booking::LEVEL_SPAN);
            ui.tooltip(ui.ctx.mouse_x, ui.ctx.mouse_y, &tip);
        }
        if near != self.near_level {
            self.near_level = near;
            self.new_search();
        }

        let search = Rect::new(x + WIN_W - PAD - BTN_W, y + (TOOL_H - BTN_H) / 2.0, BTN_W, BTN_H);
        if ui.button(SEARCH_BTN_ID, search, &rust_i18n::t!("party_board.search")).clicked() {
            self.status = None;
            self.new_search();
        }
    }

    fn draw_results(&mut self, ui: &mut UiFrame, well: Rect, events: &mut Vec<GameEvent>) {
        quad(ui, well.x - 1.0, well.y - 1.0, well.w + 2.0, well.h + 2.0, P::WIN_BORDER);
        quad(ui, well.x, well.y, well.w, well.h, P::SLOT_FILL);

        // Header.
        quad(ui, well.x, well.y, well.w, HEAD_H, HEAD_FILL);
        quad(ui, well.x, well.y + HEAD_H - 1.0, well.w, 1.0, RULE);
        let titles = [
            rust_i18n::t!("party_board.col_leader"),
            rust_i18n::t!("party_board.col_level"),
            rust_i18n::t!("party_board.col_place"),
            rust_i18n::t!("party_board.col_wanted"),
            rust_i18n::t!("party_board.col_posted"),
        ];
        for (col, title) in titles.iter().enumerate() {
            let (left, right) = (well.x + COLUMNS[col], well.x + COLUMNS[col + 1]);
            let baseline = well.y + BASELINE;
            if is_numeric(col) {
                let w = ui.atlas.measure_text(title);
                for dx in [0.0, 1.0] {
                    ui.text(right - 5.0 - w + dx - 1.0, baseline, title, text_color());
                }
            } else {
                for dx in [0.0, 1.0] {
                    clipped_text(ui, left + 4.0 + dx, baseline, title, text_color(), right - 2.0);
                }
            }
            if col > 0 {
                quad(ui, left, well.y + 2.0, 1.0, HEAD_H - 4.0, RULE);
            }
        }

        let body_y = well.y + HEAD_H;
        let note = if self.searching && self.ads.is_empty() {
            Some(rust_i18n::t!("party_board.searching"))
        } else if self.ads.is_empty() {
            Some(rust_i18n::t!("party_board.no_ads"))
        } else {
            None
        };
        if let Some(note) = note {
            ui.text(well.x + 6.0, body_y + BASELINE + 2.0, &note, MUTED_COLOR);
            return;
        }

        let now = unix_now();
        let mut whisper = None;
        let mut select = None;
        for (slot, ad) in self.ads.iter().take(RESULTS_PER_PAGE).enumerate() {
            let row = Rect::new(well.x, body_y + slot as f32 * ROW_H, well.w, ROW_H);
            let resp = ui.interact(WidgetId(ROW_BASE_ID + slot as u32), row);
            let mine = self.is_mine(ad);
            if mine {
                quad(ui, row.x, row.y, row.w, row.h, OWN_ROW);
            } else if slot % 2 == 1 {
                quad(ui, row.x, row.y, row.w, row.h, STRIPE);
            }
            if self.selected == Some(ad.index) {
                quad(ui, row.x, row.y, row.w, row.h, SELECTED_ROW);
            } else if resp.hovered() {
                quad(ui, row.x, row.y, row.w, row.h, HOVER_ROW);
            }
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                select = Some(ad.index);
            }
            if resp.double_clicked() && !mine {
                whisper = Some(ad.leader.clone());
            }

            let wanted = wanted_text(ad);
            let cells = [
                ad.leader.clone(),
                ad.level.to_string(),
                place_name(ad.place),
                wanted.clone(),
                age_label(now, ad.posted),
            ];
            let baseline = row.y + BASELINE;
            for (col, cell) in cells.iter().enumerate() {
                let (left, right) = (well.x + COLUMNS[col], well.x + COLUMNS[col + 1]);
                let color = if col == 0 && !mine { LINK_COLOR } else { text_color() };
                if is_numeric(col) {
                    ui.text_right(right - 5.0, baseline, cell, color);
                } else {
                    clipped_text(ui, left + 4.0, baseline, cell, color, right - 3.0);
                }
                if col > 0 {
                    quad(ui, left, row.y, 1.0, row.h, RULE);
                }
            }
            if resp.hovered() {
                let wanted_left = well.x + COLUMNS[3] + 4.0;
                let wanted_right = well.x + COLUMNS[4] - 3.0;
                let tip = if mine {
                    rust_i18n::t!("party_board.your_ad").into_owned()
                } else if ui.atlas.measure_text(&wanted) > wanted_right - wanted_left {
                    rust_i18n::t!("party_board.row_tip_wanted", wanted = wanted.as_str()).into_owned()
                } else {
                    rust_i18n::t!("party_board.row_tip").into_owned()
                };
                ui.tooltip(ui.ctx.mouse_x, row.y, &tip);
            }
            quad(ui, row.x, row.y + row.h - 1.0, row.w, 1.0, RULE);
        }
        if select.is_some() {
            self.selected = select;
        }
        if let Some(name) = whisper {
            events.push(GameEvent::RequestWhisper { name });
        }
    }

    fn draw_form(&mut self, ui: &mut UiFrame, area: Rect, boxes: &mut Vec<(Picker, Rect)>) {
        let mut y = area.y + 4.0;
        let (summary, color) = match &self.my_ad {
            Some(ad) => (
                rust_i18n::t!(
                    "party_board.your_ad_summary",
                    level = ad.level,
                    place = place_name(ad.place),
                    wanted = wanted_text(ad),
                    age = age_label(unix_now(), ad.posted)
                )
                .into_owned(),
                text_color(),
            ),
            None if self.unseen_ad => (rust_i18n::t!("party_board.unseen_ad").into_owned(), ERROR_COLOR),
            None => (rust_i18n::t!("party_board.no_ad").into_owned(), MUTED_COLOR),
        };
        clipped_text(ui, area.x + 2.0, y + BASELINE, &summary, color, area.x + area.w);
        y += 26.0;

        let place_text = rust_i18n::t!("party_board.place");
        ui.text(area.x + 2.0, y + BASELINE, &place_text, label_color());
        let place_rect = Rect::new(area.x + 2.0 + ui.atlas.measure_text(&place_text) + 6.0, y, 150.0, BOX_H);
        self.picker_box(ui, Picker::FormPlace, place_rect, &place_name(self.form_place));
        boxes.push((Picker::FormPlace, place_rect));
        let level_text = rust_i18n::t!("party_board.posted_level", level = self.my_level);
        ui.text(place_rect.x + place_rect.w + 12.0, y + BASELINE, &level_text, MUTED_COLOR);
        y += BOX_H + 12.0;

        let wanted_text = rust_i18n::t!("party_board.looking_for", count = BOOKING_JOBS);
        ui.text(area.x + 2.0, y + BASELINE, &wanted_text, label_color());
        let clear = Rect::new(area.x + area.w - BTN_W, y, BTN_W, BTN_H - 2.0);
        if !self.form_jobs.is_empty()
            && ui.button(CLEAR_JOBS_ID, clear, &rust_i18n::t!("party_board.clear")).clicked()
        {
            self.form_jobs.clear();
        }
        y += BTN_H + 4.0;

        let chip_w = (area.w - CHIP_GAP * (CHIP_COLS as f32 - 1.0)) / CHIP_COLS as f32;
        let mut clicked = None;
        for (n, (job, name)) in WANTED_JOBS.iter().enumerate() {
            let col = n % CHIP_COLS;
            let row = n / CHIP_COLS;
            let chip = Rect::new(
                area.x + col as f32 * (chip_w + CHIP_GAP),
                y + row as f32 * (CHIP_H + CHIP_GAP),
                chip_w,
                CHIP_H,
            );
            let on = self.form_jobs.contains(job);
            let resp = ui.interact(WidgetId(CHIP_BASE_ID + n as u32), chip);
            quad(ui, chip.x, chip.y, chip.w, chip.h, if resp.hovered() && !on { HOVER_ROW } else if on { CHIP_ON } else { CHIP_OFF });
            border(ui, chip, if on { P::GLOSS_RIM } else { RULE });
            let w = ui.atlas.measure_text(name);
            ui.text(chip.x + (chip.w - w) / 2.0, chip.y + BASELINE, name, text_color());
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                clicked = Some(*job);
            }
        }
        if let Some(job) = clicked {
            self.toggle_job(job);
        }
    }

    fn draw_footer_row(&mut self, ui: &mut UiFrame, x: f32, y: f32, events: &mut Vec<GameEvent>) {
        let by = y + (FOOTER_H - BTN_H) / 2.0;
        let mut right = x + WIN_W - PAD;
        // Buttons line up leftwards from the right edge.
        let button = |ui: &mut UiFrame, right: &mut f32, id: WidgetId, w: f32, label: &str| {
            let rect = Rect::new(*right - w, by, w, BTN_H);
            *right = rect.x - 3.0;
            ui.button(id, rect, label).clicked()
        };
        match self.view {
            View::Board => {
                if button(ui, &mut right, MY_AD_BTN_ID, BTN_W, &rust_i18n::t!("party_board.my_ad")) {
                    self.open_form();
                    return;
                }
                let selected = self
                    .selected
                    .and_then(|index| self.ads.iter().find(|ad| ad.index == index))
                    .filter(|ad| !self.is_mine(ad))
                    .map(|ad| ad.leader.clone());
                if button(ui, &mut right, WHISPER_BTN_ID, BTN_W, &rust_i18n::t!("party_board.whisper")) {
                    match selected {
                        Some(name) => events.push(GameEvent::RequestWhisper { name }),
                        None => {
                            self.status = Some(Status::Error(rust_i18n::t!("party_board.pick_an_ad").into_owned()));
                        }
                    }
                }
                right -= 6.0;
                if self.more
                    && !self.searching
                    && button(ui, &mut right, NEXT_BTN_ID, NAV_W, ">")
                    && let Some(last) = self.ads.iter().map(|ad| ad.index).max()
                {
                    self.pages.push(last + 1);
                    self.search_next_frame = true;
                }
                if !self.pages.is_empty() && !self.searching && button(ui, &mut right, PREV_BTN_ID, NAV_W, "<") {
                    self.pages.pop();
                    self.search_next_frame = true;
                }
            }
            View::MyAd => {
                let jobs = job_slots(&self.form_jobs);
                if self.my_ad.is_some() || self.unseen_ad {
                    if self.my_ad.is_some()
                        && button(ui, &mut right, POST_BTN_ID, BTN_W, &rust_i18n::t!("party_board.save"))
                    {
                        let ad = self.my_ad.as_ref().unwrap();
                        if ad.place == self.form_place {
                            events.push(GameEvent::RequestPartyBooking(BookingRequest::Update { jobs }));
                        } else {
                            // Only the jobs of an ad can change; it moves by going up again.
                            self.repost = Some(BookingRequest::Register {
                                level: self.my_level,
                                place: self.form_place,
                                jobs,
                            });
                            events.push(GameEvent::RequestPartyBooking(BookingRequest::Delete));
                        }
                    }
                    if button(ui, &mut right, REMOVE_BTN_ID, BTN_W, &rust_i18n::t!("party_board.remove")) {
                        self.repost = None;
                        events.push(GameEvent::RequestPartyBooking(BookingRequest::Delete));
                    }
                } else if button(ui, &mut right, POST_BTN_ID, BTN_W, &rust_i18n::t!("party_board.post")) {
                    events.push(GameEvent::RequestPartyBooking(BookingRequest::Register {
                        level: self.my_level,
                        place: self.form_place,
                        jobs,
                    }));
                }
                let back = Rect::new(x + PAD, by, BTN_W, BTN_H);
                if ui.button(BACK_BTN_ID, back, &rust_i18n::t!("party_board.back")).clicked() {
                    self.view = View::Board;
                    self.status = None;
                    self.close_picker();
                    return;
                }
            }
        }
        let left = if self.view == View::MyAd { x + PAD + BTN_W + 8.0 } else { x + PAD };
        let (text, color) = match &self.status {
            Some(Status::Info(text)) => (text.clone(), text_color()),
            Some(Status::Error(text)) => (text.clone(), ERROR_COLOR),
            None if self.view == View::Board && self.searching => {
                (rust_i18n::t!("party_board.searching").into_owned(), MUTED_COLOR)
            }
            None if self.view == View::Board => {
                let page = self.pages.len() + 1;
                (rust_i18n::t!("party_board.count", count = self.ads.len(), page = page).into_owned(), MUTED_COLOR)
            }
            None => (rust_i18n::t!("party_board.form_hint").into_owned(), MUTED_COLOR),
        };
        clipped_text(ui, left, y + FOOTER_H / 2.0 + 4.0, &text, color, right - 4.0);
    }

    /// The open picker's choices, over everything else.
    fn draw_picker(&mut self, ui: &mut UiFrame, anchor: Rect) {
        let Some(which) = self.picker else {
            return;
        };
        let labels: Vec<String> = match which {
            Picker::Place | Picker::FormPlace => PLACES.iter().map(|p| (*p).to_string()).collect(),
            Picker::Job => std::iter::once(rust_i18n::t!("party_board.any_job").into_owned())
                .chain(WANTED_JOBS.iter().map(|(_, name)| (*name).to_string()))
                .collect(),
        };
        let current = match which {
            Picker::Place => self.place as usize,
            Picker::FormPlace => self.form_place as usize,
            Picker::Job => WANTED_JOBS.iter().position(|(id, _)| *id == self.job).map_or(0, |n| n + 1),
        };
        let rows = labels.len().div_ceil(PICK_COLS);
        let w = PICK_COL_W * PICK_COLS as f32;
        let h = rows as f32 * OPTION_H + 4.0;
        let x = anchor.x.min(ui.ctx.screen_width - w - 2.0).max(0.0);
        let below = anchor.y + anchor.h + 1.0;
        let y = if below + h > ui.ctx.screen_height { (anchor.y - h - 1.0).max(0.0) } else { below };
        let rect = Rect::new(x, y, w, h);

        ui.begin_popup_layer(rect);
        quad(ui, rect.x, rect.y, rect.w, rect.h, P::SLOT_FILL);
        border(ui, rect, P::FIELD_RIM);
        let mut picked = None;
        for (n, label) in labels.iter().enumerate() {
            let cell = Rect::new(
                rect.x + (n / rows) as f32 * PICK_COL_W,
                rect.y + 2.0 + (n % rows) as f32 * OPTION_H,
                PICK_COL_W,
                OPTION_H,
            );
            let resp = ui.interact(WidgetId(PICK_BASE_ID + n as u32), cell);
            if n == current {
                quad(ui, cell.x + 1.0, cell.y, cell.w - 2.0, cell.h, CHIP_ON);
            } else if resp.hovered() {
                quad(ui, cell.x + 1.0, cell.y, cell.w - 2.0, cell.h, HOVER_ROW);
            }
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            clipped_text(ui, cell.x + 5.0, cell.y + 12.0, label, text_color(), cell.x + cell.w - 3.0);
            if resp.clicked() {
                picked = Some(n);
            }
        }
        ui.end_popup_layer();

        let clicked_away = ui.ctx.mouse_clicked
            && !rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y)
            && !anchor.contains(ui.ctx.mouse_x, ui.ctx.mouse_y);
        if let Some(n) = picked {
            self.close_picker();
            match which {
                Picker::Place => {
                    self.place = n as i16;
                    // The server filters by place or by job, not both.
                    if self.place != 0 {
                        self.job = NO_JOB;
                    }
                    self.new_search();
                }
                Picker::Job => {
                    self.job = if n == 0 { NO_JOB } else { WANTED_JOBS[n - 1].0 };
                    if self.job != NO_JOB {
                        self.place = 0;
                    }
                    self.new_search();
                }
                Picker::FormPlace => self.form_place = n as i16,
            }
        } else if clicked_away {
            self.close_picker();
        }
    }
}

impl Window for PartyBoardWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for PartyBoardWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if self.picker.is_some() {
            self.close_picker();
        } else {
            self.open = false;
        }
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        self.me.clone_from(&ctx.character.name);
        self.my_level = i16::try_from(ctx.character.base_level).unwrap_or(i16::MAX);
        let mut events = std::mem::take(&mut self.outbox);
        if std::mem::take(&mut self.search_next_frame) {
            self.searching = true;
            let from = self.pages.last().copied().unwrap_or(0);
            events.push(GameEvent::RequestPartyBooking(BookingRequest::Search { filter: self.filter(), from }));
        }
        self.place_box.begin_frame();
        self.job_box.begin_frame();
        self.form_box.begin_frame();

        let win = ui.window_at(PARTY_BOARD_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 180.0, 120.0);
        let (x, y) = (win.x, win.y);
        ui.interact(PARTY_BOARD_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H);
        ui.text(x + 16.0, y + 13.0, &rust_i18n::t!("party_board.title"), text_color());
        let close = Rect::new(x + WIN_W - 3.0 - SYS_BTN, y + 3.0, SYS_BTN, SYS_BTN);
        let close_resp = ui.interact(CLOSE_BTN_ID, close);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(ui, close, (SYS_BTN, SYS_BTN), close_resp.hovered(), Some('x'));
        if close_resp.clicked() {
            self.open = false;
            self.close_picker();
            return events;
        }

        let body_y = y + TITLE_H;
        draw_container(ui, x, body_y, WIN_W, TOOL_H + BODY_H + WELL_GAP);
        let mut boxes = Vec::new();
        match self.view {
            View::Board => {
                self.draw_toolbar(ui, x, body_y, &mut boxes);
                let well = Rect::new(x + PAD, body_y + TOOL_H, WIN_W - PAD * 2.0, BODY_H);
                self.draw_results(ui, well, &mut events);
            }
            View::MyAd => {
                let area = Rect::new(x + PAD + 2.0, body_y + 2.0, WIN_W - PAD * 2.0 - 4.0, TOOL_H + BODY_H);
                self.draw_form(ui, area, &mut boxes);
            }
        }

        let footer_y = body_y + TOOL_H + BODY_H + WELL_GAP;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H);
        self.draw_footer_row(ui, x, footer_y, &mut events);

        if let Some(which) = self.picker
            && let Some((_, anchor)) = boxes.iter().find(|(picker, _)| *picker == which)
        {
            let anchor = *anchor;
            self.draw_picker(ui, anchor);
        } else if self.picker.is_some() {
            // Its box went away with the view.
            self.close_picker();
        }
        self.boxes = boxes;
        events
    }
}

fn is_numeric(col: usize) -> bool {
    col == 1 || col == 4
}

/// "Priest, Wizard", or "Anyone" for an ad that names no job.
fn wanted_text(ad: &BookingAd) -> String {
    let jobs: Vec<String> = ad.wanted().map(job_label).collect();
    if jobs.is_empty() {
        rust_i18n::t!("party_board.anyone").into_owned()
    } else {
        jobs.join(", ")
    }
}

fn border(ui: &mut UiFrame, r: Rect, color: [f32; 4]) {
    quad(ui, r.x, r.y, r.w, 1.0, color);
    quad(ui, r.x, r.y + r.h - 1.0, r.w, 1.0, color);
    quad(ui, r.x, r.y, 1.0, r.h, color);
    quad(ui, r.x + r.w - 1.0, r.y, 1.0, r.h, color);
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    const ORIGIN: (f32, f32) = (180.0, 120.0);

    fn ad(index: u32, leader: &str, place: i16, jobs: &[i16]) -> BookingAd {
        BookingAd { index, leader: leader.into(), posted: 0, level: 50, place, jobs: job_slots(jobs) }
    }

    struct Harness {
        win: PartyBoardWindow,
        state: StateCache,
        character: Character,
        data: DataTable,
    }

    impl Harness {
        fn new() -> Self {
            let mut character = Character::new();
            character.name = "Me".into();
            character.base_level = 55;
            let mut win = PartyBoardWindow::new();
            win.toggle();
            Self { win, state: StateCache::new(), character, data: DataTable::new() }
        }

        /// One frame with the pointer at `at` (relative to the window), clicked or not.
        fn frame(&mut self, at: Option<(f32, f32)>, clicked: bool, double: bool) -> Vec<GameEvent> {
            let mut ctx = UiContext::new(1024.0, 768.0);
            if let Some((dx, dy)) = at {
                ctx.mouse_x = ORIGIN.0 + dx;
                ctx.mouse_y = ORIGIN.1 + dy;
            }
            ctx.mouse_clicked = clicked;
            ctx.mouse_double_clicked = double;
            let mut ui = test_frame(&mut ctx, &mut self.state);
            self.win.build(&mut ui, &mut crate::BuildCtx::test(&mut self.character, &self.data))
        }

        fn idle(&mut self) -> Vec<GameEvent> {
            self.frame(None, false, false)
        }

        fn click(&mut self, at: (f32, f32)) -> Vec<GameEvent> {
            self.frame(Some(at), true, false)
        }
    }

    fn requests(events: &[GameEvent]) -> Vec<BookingRequest> {
        events
            .iter()
            .filter_map(|e| match e {
                GameEvent::RequestPartyBooking(request) => Some(request.clone()),
                _ => None,
            })
            .collect()
    }

    fn row_center(slot: usize) -> (f32, f32) {
        (PAD + 40.0, TITLE_H + TOOL_H + HEAD_H + slot as f32 * ROW_H + ROW_H / 2.0)
    }

    fn footer_button(n: usize, w_before: &[f32]) -> (f32, f32) {
        // Footer buttons line up from the right edge, `n` of them before.
        let mut right = WIN_W - PAD;
        for w in &w_before[..n] {
            right -= w + 3.0;
        }
        (right - 10.0, TITLE_H + TOOL_H + BODY_H + WELL_GAP + FOOTER_H / 2.0)
    }

    #[test]
    fn opening_searches_the_first_page_for_everyone() {
        let mut h = Harness::new();
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Search { filter: BookingFilter::default(), from: 0 }]
        );
        assert!(requests(&h.idle()).is_empty(), "asks once");
    }

    #[test]
    fn results_fill_the_table_and_find_the_own_ad() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![ad(1, "Lead", 4, &[8]), ad(2, "Me", 0, &[])], more: false });
        assert_eq!(h.win.ads.len(), 2);
        assert_eq!(h.win.my_ad.as_ref().map(|a| a.index), Some(2));
        assert!(!h.win.searching);
        assert_eq!(wanted_text(&h.win.ads[0]), "Priest");
        assert_eq!(wanted_text(&h.win.ads[1]), "Anyone");
    }

    #[test]
    fn double_clicking_an_ad_whispers_its_leader() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![ad(1, "Lead", 4, &[8])], more: false });
        let events = h.frame(Some(row_center(0)), true, true);
        assert!(events.iter().any(|e| matches!(e, GameEvent::RequestWhisper { name } if name == "Lead")));
        assert_eq!(h.win.selected, Some(1));
    }

    #[test]
    fn whisper_button_needs_a_selected_ad() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![ad(1, "Lead", 4, &[8])], more: false });
        let whisper = footer_button(1, &[BTN_W]);
        let events = h.click(whisper);
        assert!(!events.iter().any(|e| matches!(e, GameEvent::RequestWhisper { .. })));
        assert!(matches!(h.win.status, Some(Status::Error(_))));
        h.click(row_center(0));
        let events = h.click(whisper);
        assert!(events.iter().any(|e| matches!(e, GameEvent::RequestWhisper { name } if name == "Lead")));
    }

    #[test]
    fn next_page_starts_after_the_last_ad_and_back_returns() {
        let mut h = Harness::new();
        h.idle();
        let page: Vec<BookingAd> = (1..=10).map(|i| ad(i, &format!("P{i}"), 0, &[])).collect();
        h.win.deliver(BookingNotice::Results { ads: page, more: true });
        let next = footer_button(2, &[BTN_W, BTN_W + 6.0]);
        h.click(next);
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Search { filter: BookingFilter::default(), from: 11 }]
        );
        h.win.deliver(BookingNotice::Results { ads: vec![ad(11, "P11", 0, &[])], more: false });
        // No "more": the "<" button takes the place of ">".
        let prev = footer_button(2, &[BTN_W, BTN_W + 6.0]);
        h.click(prev);
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Search { filter: BookingFilter::default(), from: 0 }]
        );
    }

    #[test]
    fn picking_a_job_clears_the_place_and_searches_again() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![], more: false });
        h.win.place = 4;
        let anchor = h.win.boxes.iter().find(|(p, _)| *p == Picker::Job).unwrap().1;
        h.click((anchor.x + 10.0 - ORIGIN.0, anchor.y + 8.0 - ORIGIN.1));
        assert_eq!(h.win.picker, Some(Picker::Job));
        h.idle();
        // Choices run down the columns, "Any" first.
        let rows = (WANTED_JOBS.len() + 1).div_ceil(PICK_COLS);
        let priest = WANTED_JOBS.iter().position(|(id, _)| *id == 8).unwrap() + 1;
        let cell_x = anchor.x + (priest / rows) as f32 * PICK_COL_W + 20.0 - ORIGIN.0;
        let cell_y = anchor.y + anchor.h + 1.0 + 2.0 + (priest % rows) as f32 * OPTION_H + 8.0 - ORIGIN.1;
        h.click((cell_x, cell_y));
        assert_eq!(h.win.picker, None);
        assert_eq!((h.win.place, h.win.job), (0, 8));
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Search { filter: BookingFilter { level: 0, place: 0, job: 8 }, from: 0 }]
        );
    }

    #[test]
    fn near_my_level_searches_with_the_character_level() {
        let mut h = Harness::new();
        h.idle();
        h.win.near_level = true;
        h.win.new_search();
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Search { filter: BookingFilter { level: 55, place: 0, job: NO_JOB }, from: 0 }]
        );
    }

    #[test]
    fn live_notices_keep_the_list_current() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![ad(1, "Lead", 4, &[8])], more: false });
        h.win.deliver(BookingNotice::Inserted(ad(2, "New", 0, &[])));
        h.win.deliver(BookingNotice::Inserted(ad(2, "New", 0, &[])));
        assert_eq!(h.win.ads.iter().map(|a| a.index).collect::<Vec<_>>(), vec![1, 2]);
        h.win.deliver(BookingNotice::Updated { index: 1, jobs: job_slots(&[9]) });
        assert_eq!(h.win.ads[0].wanted().collect::<Vec<_>>(), vec![9]);
        h.win.deliver(BookingNotice::Removed { index: 1 });
        assert_eq!(h.win.ads.iter().map(|a| a.index).collect::<Vec<_>>(), vec![2]);

        // An ad the filter would not find stays out.
        h.win.job = 8;
        h.win.deliver(BookingNotice::Inserted(ad(3, "Other", 0, &[7])));
        assert!(!h.win.ads.iter().any(|a| a.index == 3));
    }

    #[test]
    fn posting_an_ad_sends_the_form() {
        let mut h = Harness::new();
        h.idle();
        h.win.open_form();
        h.win.form_place = 5;
        h.win.toggle_job(8);
        h.win.toggle_job(9);
        h.win.toggle_job(8);
        let events = h.click(footer_button(0, &[]));
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Register { level: 55, place: 5, jobs: job_slots(&[9]) }]
        );
        h.win.deliver(BookingNotice::Registered { ok: true });
        h.win.deliver(BookingNotice::Inserted(BookingAd { level: 55, ..ad(7, "Me", 5, &[9]) }));
        assert_eq!(h.win.my_ad.as_ref().map(|a| a.index), Some(7));
    }

    #[test]
    fn a_form_takes_six_jobs_at_most() {
        let mut win = PartyBoardWindow::new();
        for (job, _) in WANTED_JOBS.iter().take(7) {
            win.toggle_job(*job);
        }
        assert_eq!(win.form_jobs.len(), BOOKING_JOBS);
        assert!(matches!(win.status, Some(Status::Error(_))));
    }

    #[test]
    fn saving_changes_the_jobs_or_moves_the_ad() {
        let mut h = Harness::new();
        h.idle();
        h.win.deliver(BookingNotice::Results { ads: vec![ad(7, "Me", 5, &[9])], more: false });
        h.win.open_form();
        assert_eq!((h.win.form_place, h.win.form_jobs.clone()), (5, vec![9]));

        h.win.toggle_job(8);
        let save = footer_button(0, &[]);
        let events = h.click(save);
        assert_eq!(requests(&events), vec![BookingRequest::Update { jobs: job_slots(&[9, 8]) }]);

        h.win.form_place = 6;
        let events = h.click(save);
        assert_eq!(requests(&events), vec![BookingRequest::Delete]);
        h.win.deliver(BookingNotice::Deleted { ok: true });
        let events = h.idle();
        assert_eq!(
            requests(&events),
            vec![BookingRequest::Register { level: 55, place: 6, jobs: job_slots(&[9, 8]) }]
        );
    }

    #[test]
    fn an_ad_from_before_a_relog_can_be_taken_down() {
        let mut h = Harness::new();
        h.idle();
        h.win.open_form();
        h.win.deliver(BookingNotice::Registered { ok: false });
        assert!(h.win.unseen_ad);
        let events = h.click(footer_button(0, &[]));
        assert_eq!(requests(&events), vec![BookingRequest::Delete]);
        h.win.deliver(BookingNotice::Deleted { ok: true });
        assert!(!h.win.unseen_ad);
    }

    #[test]
    fn escape_closes_the_picker_before_the_window() {
        let mut h = Harness::new();
        h.idle();
        h.win.picker = Some(Picker::Place);
        h.win.place_box.open = true;
        let mut ctx = crate::BuildCtx::test(&mut h.character, &h.data);
        h.win.on_escape(&mut ctx);
        assert!(h.win.is_open());
        assert_eq!(h.win.picker, None);
        h.win.on_escape(&mut ctx);
        assert!(!h.win.is_open());
    }
}
