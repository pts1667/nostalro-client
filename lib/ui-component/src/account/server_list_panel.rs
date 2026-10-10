//! The server picker shared by the post-login server list and the login server
//! list: a titled light panel, one row per server, and OK / Cancel beneath.

use crate::helper::fallback;
use crate::helper::window_chrome::{draw_container, draw_titlebar, label_color, text_color};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::theme::FallbackPalette as P;

const WIN_W: f32 = 280.0;
const TITLE_BAR_H: f32 = 30.0;
const ROW_H: f32 = 17.0;
const PADDING: f32 = 8.0;
const BTN_W: f32 = 42.0;
const BTN_H: f32 = 20.0;
const BTN_SPACING: f32 = 8.0;

/// Ids and title for one picker. `window` is also the base for the row ids.
pub struct PanelSpec<'a> {
    pub window: WidgetId,
    pub ok: WidgetId,
    pub cancel: WidgetId,
    pub title: &'a str,
}

/// One row: the name on the left, a short detail on the right.
pub struct Row<'a> {
    pub name: &'a str,
    pub detail: &'a str,
}

/// Which button was clicked this frame. Row selection is written to the caller's
/// `selected` index directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PanelOutput {
    pub ok: bool,
    pub cancel: bool,
}

/// Outer size of a picker with `rows` rows.
pub fn size(rows: usize) -> (f32, f32) {
    (
        WIN_W,
        TITLE_BAR_H + rows as f32 * ROW_H + PADDING + BTN_H + PADDING,
    )
}

pub fn build(
    ui: &mut UiFrame,
    spec: PanelSpec,
    rows: &[Row],
    selected: &mut Option<usize>,
) -> PanelOutput {
    let (win_w, win_h) = size(rows.len());
    let win = ui.window_account(spec.window, win_w, win_h, TITLE_BAR_H);
    draw_titlebar(ui, win.x, win.y, win.w, TITLE_BAR_H);
    draw_container(ui, win.x, win.y + TITLE_BAR_H, win.w, win_h - TITLE_BAR_H);

    let title_w = ui.atlas.measure_text(spec.title);
    let title_y = win.y + (TITLE_BAR_H + ui.atlas.line_height) / 2.0 - 3.0;
    ui.text(
        win.x + (win.w - title_w) / 2.0,
        title_y,
        spec.title,
        text_color(),
    );

    let list_y = win.y + TITLE_BAR_H;
    for (idx, row) in rows.iter().enumerate() {
        let row_y = list_y + idx as f32 * ROW_H;
        let row_rect = Rect::new(win.x + PADDING, row_y, win.w - PADDING * 2.0, ROW_H);
        let hit = ui.interact(WidgetId(spec.window.0 + 10 + idx as u32), row_rect);
        if hit.hovered() {
            ui.any_interactive_hovered = true;
        }
        if hit.clicked() {
            *selected = Some(idx);
        }

        let fill = if *selected == Some(idx) {
            Some(P::BTN_HOVER_MID)
        } else if hit.hovered() {
            Some(P::BTN_HOVER_TOP)
        } else {
            None
        };
        if let Some(fill) = fill {
            fallback::fill_into(
                &mut ui.draw_calls,
                row_rect.x,
                row_rect.y,
                row_rect.w,
                row_rect.h,
                fill,
            );
        }

        let text_y = row_y + (ROW_H + ui.atlas.line_height) / 2.0 - 2.0;
        ui.text(win.x + PADDING + 4.0, text_y, row.name, text_color());
        let detail_w = ui.atlas.measure_text(row.detail);
        ui.text(
            win.x + win.w - PADDING - 4.0 - detail_w,
            text_y,
            row.detail,
            label_color(),
        );
    }

    let btn_y = list_y + rows.len() as f32 * ROW_H + PADDING;
    let total_w = BTN_W * 2.0 + BTN_SPACING;
    let start_x = win.x + (win.w - total_w) / 2.0;
    let ok = ui.button(
        spec.ok,
        Rect::new(start_x, btn_y, BTN_W, BTN_H),
        &rust_i18n::t!("common.ok"),
    );
    let cancel = ui.button(
        spec.cancel,
        Rect::new(start_x + BTN_W + BTN_SPACING, btn_y, BTN_W, BTN_H),
        &rust_i18n::t!("common.cancel"),
    );

    PanelOutput {
        ok: ok.clicked(),
        cancel: cancel.clicked(),
    }
}
