//! The travel window: the towns a trip can take, and the Go button that starts
//! it. The client fills the list, says what the status line reads and whether a
//! trip is under way; a world click or the Stop button ends the trip.

use crate::helper::window_chrome::{draw_sys_button, draw_titlebar, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

pub const TRAVEL_WINDOW_ID: WidgetId = WidgetId(6600);
const CLOSE_BTN_ID: WidgetId = WidgetId(6601);
const GO_BTN_ID: WidgetId = WidgetId(6602);
/// The rows of the list take ids from here, one per visible row.
const ROW_BASE_ID: u32 = 6610;

const WIN_W: f32 = 240.0;
const TITLE_H: f32 = 20.0;
const CLOSE_SIZE: f32 = 11.0;
const PAD: f32 = 8.0;
const STATUS_H: f32 = 16.0;
const ROW_H: f32 = 18.0;
const VISIBLE_ROWS: usize = 12;
const LIST_H: f32 = VISIBLE_ROWS as f32 * ROW_H;
const BTN_H: f32 = 22.0;
const WIN_H: f32 = TITLE_H + PAD + STATUS_H + LIST_H + PAD + BTN_H + PAD;
const SELECTED_COLOR: [f32; 4] = [0.3, 0.3, 0.5, 0.5];

/// A town on the list: its name, and the map a trip to it ends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelTown {
    pub name: String,
    pub map: String,
}

pub struct TravelWindow {
    pub open: bool,
    /// The towns to list, copied in by the client.
    pub towns: Vec<TravelTown>,
    /// The line above the list: loading, an error, or how to use the window.
    pub status: String,
    /// Whether a trip is under way; the Go button then stops it.
    pub travelling: bool,
    selected: usize,
    /// The first town on screen.
    first: usize,
}

impl Default for TravelWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl TravelWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            towns: Vec::new(),
            status: String::new(),
            travelling: false,
            selected: 0,
            first: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// The town the Go button travels to.
    pub fn selected_town(&self) -> Option<&TravelTown> {
        self.towns.get(self.selected)
    }

    /// Moves the list by `rows`, keeping the first town on screen within the list.
    fn scroll_by(&mut self, rows: isize) {
        let most = self.towns.len().saturating_sub(VISIBLE_ROWS);
        self.first = (self.first as isize + rows).clamp(0, most as isize) as usize;
    }
}

impl Window for TravelWindow {
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
}

impl InGameWindow for TravelWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let default_x = ((ui.ctx.screen_width - WIN_W) / 2.0).floor();
        let default_y = ((ui.ctx.screen_height - WIN_H) / 2.0).floor();
        let win = ui.window_at(TRAVEL_WINDOW_ID, WIN_W, WIN_H, TITLE_H, default_x, default_y);
        ui.interact(TRAVEL_WINDOW_ID, Rect::new(win.x, win.y, WIN_W, WIN_H));

        crate::helper::fallback::window_body(ui, win.x, win.y + TITLE_H, WIN_W, WIN_H - TITLE_H);
        draw_titlebar(ui, win.x, win.y, WIN_W, TITLE_H);
        ui.text(win.x + 20.0, win.y + TITLE_H - 5.0, "Travel", text_color());

        let close_rect = Rect::new(
            win.x + WIN_W - CLOSE_SIZE - 4.0,
            win.y + 4.0,
            CLOSE_SIZE,
            CLOSE_SIZE,
        );
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(ui, close_rect, (CLOSE_SIZE, CLOSE_SIZE), close_resp.hovered(), Some('x'));
        if close_resp.clicked() {
            self.open = false;
            return events;
        }

        let status_y = win.y + TITLE_H + PAD;
        ui.text(win.x + PAD, status_y + 11.0, &self.status, text_color());

        let list_y = status_y + STATUS_H;
        let list = Rect::new(win.x + PAD, list_y, WIN_W - PAD * 2.0, LIST_H);
        let wheel = ui.take_scroll(list);
        if wheel > 0.0 {
            self.scroll_by(-1);
        } else if wheel < 0.0 {
            self.scroll_by(1);
        }
        self.scroll_by(0);

        for row in 0..VISIBLE_ROWS {
            let index = self.first + row;
            let Some(town) = self.towns.get(index) else {
                break;
            };
            let row_rect = Rect::new(list.x, list_y + row as f32 * ROW_H, list.w, ROW_H);
            let resp = ui.interact(WidgetId(ROW_BASE_ID + row as u32), row_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                self.selected = index;
            }
            if index == self.selected {
                let (v, i) = draw::quad_vertices(
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    SELECTED_COLOR,
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: i.to_vec(),
                    texture: TextureRef::White,
                });
            }
            ui.text(row_rect.x + 4.0, row_rect.y + ROW_H - 4.0, &town.name, text_color());
        }

        let go_rect = Rect::new(
            win.x + PAD,
            list_y + LIST_H + PAD,
            WIN_W - PAD * 2.0,
            BTN_H,
        );
        let label = if self.travelling { "Stop travel" } else { "Go" };
        let go = ui.button(GO_BTN_ID, go_rect, label);
        if go.clicked() {
            if self.travelling {
                events.push(GameEvent::StopTravel);
            } else if let Some(town) = self.selected_town() {
                events.push(GameEvent::StartTravel {
                    map: town.map.clone(),
                });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(n: usize) -> TravelTown {
        TravelTown {
            name: format!("Town {n}"),
            map: format!("map_{n}"),
        }
    }

    fn window_with(count: usize) -> TravelWindow {
        let mut win = TravelWindow::new();
        win.towns = (0..count).map(town).collect();
        win
    }

    #[test]
    fn go_travels_to_the_selected_town() {
        let mut win = window_with(3);
        assert_eq!(win.selected_town(), Some(&town(0)));
        win.selected = 2;
        assert_eq!(win.selected_town().map(|t| t.map.as_str()), Some("map_2"));
    }

    #[test]
    fn an_empty_list_has_no_town_to_travel_to() {
        assert!(window_with(0).selected_town().is_none());
    }

    #[test]
    fn the_list_scrolls_but_stays_within_its_towns() {
        let mut win = window_with(VISIBLE_ROWS + 3);
        win.scroll_by(10);
        assert_eq!(win.first, 3);
        win.scroll_by(-10);
        assert_eq!(win.first, 0);
        let mut short = window_with(4);
        short.scroll_by(5);
        assert_eq!(short.first, 0);
    }
}
