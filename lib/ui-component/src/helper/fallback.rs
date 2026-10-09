use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::UiFrame;
use ragnarok_ui::rect::Rect;
use ragnarok_ui::theme::{CORNER_RADIUS, FallbackPalette as P};

fn fill(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (v, i) = draw::quad_vertices(x, y, w, h, color);
    ui.draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
}

fn rounded(
    ui: &mut UiFrame,
    verts: Vec<ragnarok_renderer::ui_renderer::UiVertex>,
    indices: Vec<u32>,
) {
    ui.draw_calls.push(DrawCall {
        vertices: verts,
        indices,
        texture: TextureRef::White,
    });
}

/// Glossy blue title bar with rounded top corners, a 1px black baseline, and the
/// top-left system button (mirrors the GRF title bar).
pub fn titlebar(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let (v, i) = draw::rounded_rect_corners_vgrad(
        x,
        y,
        w,
        h,
        [CORNER_RADIUS, CORNER_RADIUS, 0.0, 0.0],
        P::TITLE_TOP,
        P::TITLE_BOT,
    );
    rounded(ui, v, i);
    fill(ui, x, y + h - 1.0, w, 1.0, P::TITLE_BASELINE);

    let (bx, by, bs) = (x + 4.0, y + 3.0, 11.0);
    let hovered = Rect::new(bx, by, bs, bs).contains(ui.ctx.mouse_x, ui.ctx.mouse_y);
    sys_button(ui, bx, by, bs, hovered, None);
}

/// Light window body (middle section) with left/right edge borders.
pub fn container(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    fill(ui, x, y, w, h, P::WIN_BODY);
    fill(ui, x, y, 1.0, h, P::WIN_BORDER);
    fill(ui, x + w - 1.0, y, 1.0, h, P::WIN_BORDER);
}

/// Light panel with rounded bottom corners and left/right/bottom borders. Square
/// top so it butts flush against a title bar. Used for footers and standalone
/// window bodies that sit directly under a title bar with no footer.
pub fn window_body(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let (v, i) = draw::rounded_rect_corners_vgrad(
        x,
        y,
        w,
        h,
        [0.0, 0.0, CORNER_RADIUS, CORNER_RADIUS],
        P::WIN_BODY,
        P::WIN_BODY,
    );
    rounded(ui, v, i);
    fill(ui, x, y, 1.0, h, P::WIN_BORDER);
    fill(ui, x + w - 1.0, y, 1.0, h, P::WIN_BORDER);
    fill(
        ui,
        x + CORNER_RADIUS,
        y + h - 1.0,
        w - 2.0 * CORNER_RADIUS,
        1.0,
        P::WIN_BORDER,
    );
}

/// Light footer with rounded bottom corners and a gray edge border.
pub fn footer(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    window_body(ui, x, y, w, h);
}

/// Standalone light panel (all corners rounded) with a gray border — for
/// windows that draw their own body without a separate title bar.
pub fn panel(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    panel_into(&mut ui.draw_calls, x, y, w, h);
}

/// [`panel`] for code that builds its draw calls without a `UiFrame`.
/// Solid rectangle with the white texture, for glyphs drawn outside a window.
pub fn fill_into(draw_calls: &mut Vec<DrawCall>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (v, i) = draw::quad_vertices(x, y, w, h, color);
    draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
}

/// Solid triangle with the white texture.
pub fn triangle_into(draw_calls: &mut Vec<DrawCall>, points: [(f32, f32); 3], color: [f32; 4]) {
    let (v, i) = draw::triangle_vertices(points, color);
    draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
}

/// Map arrow pointing along `angle_rad` (0 is up, positive turns clockwise on
/// screen), with a dark edge so it reads on the map art.
pub fn arrow(
    draw_calls: &mut Vec<DrawCall>,
    cx: f32,
    cy: f32,
    size: f32,
    angle_rad: f32,
    color: [f32; 4],
) {
    const EDGE: [f32; 3] = [0.10, 0.10, 0.12];
    let edge = [EDGE[0], EDGE[1], EDGE[2], color[3]];
    triangle_into(draw_calls, arrow_points(cx, cy, size + 2.0, angle_rad), edge);
    triangle_into(draw_calls, arrow_points(cx, cy, size, angle_rad), color);
}

fn arrow_points(cx: f32, cy: f32, size: f32, angle_rad: f32) -> [(f32, f32); 3] {
    let h = size / 2.0;
    let (sin, cos) = angle_rad.sin_cos();
    let at = |x: f32, y: f32| (cx + x * cos - y * sin, cy + x * sin + y * cos);
    [at(0.0, -h), at(-h * 0.8, h * 0.8), at(h * 0.8, h * 0.8)]
}

pub fn panel_into(draw_calls: &mut Vec<DrawCall>, x: f32, y: f32, w: f32, h: f32) {
    let (vertices, indices) = draw::rounded_rect(x, y, w, h, CORNER_RADIUS, P::WIN_BORDER);
    draw_calls.push(DrawCall {
        vertices,
        indices,
        texture: TextureRef::White,
    });
    let (vertices, indices) = draw::rounded_rect(
        x + 1.0,
        y + 1.0,
        w - 2.0,
        h - 2.0,
        (CORNER_RADIUS - 1.0).max(0.0),
        P::WIN_BODY,
    );
    draw_calls.push(DrawCall {
        vertices,
        indices,
        texture: TextureRef::White,
    });
}

/// Flat light cell with a 1px border — for stat cells, tabs and selector fields.
/// `active` brightens the fill to distinguish a selected tab from the rest.
pub fn cell(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, active: bool) {
    let bg = if active { P::WIN_BODY } else { P::WIN_BODY_ALT };
    let (v, i) = draw::rounded_rect(x, y, w, h, 2.0, bg);
    rounded(ui, v, i);
    fill(ui, x, y, w, 1.0, P::WIN_BORDER);
    fill(ui, x, y + h - 1.0, w, 1.0, P::WIN_BORDER);
    fill(ui, x, y, 1.0, h, P::WIN_BORDER);
    fill(ui, x + w - 1.0, y, 1.0, h, P::WIN_BORDER);
}

/// Small rounded blue system button (close/minimize) with a dark-navy glyph.
pub fn sys_button(ui: &mut UiFrame, x: f32, y: f32, size: f32, hovered: bool, glyph: Option<char>) {
    let color = if hovered {
        P::SYS_BTN_HOVER
    } else {
        P::SYS_BTN
    };
    let (v, i) = draw::rounded_rect(x, y, size, size, 2.0, color);
    rounded(ui, v, i);
    if let Some(ch) = glyph {
        let s = ch.to_string();
        let tw = ui.atlas.measure_text(&s);
        let tx = x + (size - tw) / 2.0;
        let ty = y + size - (ui.atlas.line_height / 2.0) + 1.0;
        let (tv, ti) = draw::text_vertices(&s, tx, ty, P::SYS_GLYPH, ui.atlas);
        if !tv.is_empty() {
            ui.draw_calls.push(DrawCall {
                vertices: tv,
                indices: ti,
                texture: TextureRef::FontAtlas,
            });
        }
    }
}

/// Recessed slot cell: white with a soft blue-gray inset (inventory/equip/cart).
pub fn slot_cell(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let (v, i) = draw::rounded_rect_vgrad(x, y, w, h, 2.0, P::SLOT_FILL, P::SLOT_INSET);
    rounded(ui, v, i);
    fill(ui, x, y, w, 1.0, P::WIN_BORDER);
    fill(ui, x, y + h - 1.0, w, 1.0, P::WIN_BORDER);
    fill(ui, x, y, 1.0, h, P::WIN_BORDER);
    fill(ui, x + w - 1.0, y, 1.0, h, P::WIN_BORDER);
}

/// Rounded HP/SP fill bar with a glossy vertical gradient.
pub fn gauge(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, fill_pct: f32, is_red: bool) {
    let pct = fill_pct.clamp(0.0, 1.0);
    fill(ui, x, y, w, h, P::WIN_BODY_ALT);
    if pct <= 0.0 {
        return;
    }
    let (top, bot) = if is_red {
        ([0.82, 0.44, 0.55, 1.0], [0.90, 0.58, 0.69, 1.0])
    } else {
        ([0.44, 0.55, 0.82, 1.0], [0.61, 0.71, 0.92, 1.0])
    };
    let (v, i) = draw::rounded_rect_vgrad(x, y, w * pct, h, 2.0, top, bot);
    rounded(ui, v, i);
}

/// Scrollbar arrow button: a raised rounded face with a small triangle drawn
/// from quads, so it does not depend on a glyph being in the font.
pub fn scroll_button(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, up: bool, hovered: bool) {
    let (top, mid) = if hovered {
        (P::BTN_HOVER_TOP, P::BTN_HOVER_MID)
    } else {
        (P::BTN_FACE_TOP, P::BTN_FACE_MID)
    };
    let (v, i) = draw::rounded_rect_vgrad(x, y, w, h, 2.0, top, mid);
    rounded(ui, v, i);

    // Four rows, apex first, centred in the button.
    let cx = x + w * 0.5;
    let cy = y + h * 0.5 - 2.0;
    for k in 0..4 {
        let row_w = 1.0 + 2.0 * k as f32;
        let row = if up { k } else { 3 - k };
        fill(
            ui,
            cx - row_w * 0.5,
            cy + row as f32,
            row_w,
            1.0,
            P::TEXT_ON_LIGHT,
        );
    }
}

/// Recessed track behind a scrollbar thumb.
pub fn scroll_track(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let (v, i) = draw::rounded_rect(x, y, w, h, 2.0, P::WIN_BORDER);
    rounded(ui, v, i);
    let (v, i) = draw::rounded_rect_vgrad(
        x + 1.0,
        y + 1.0,
        w - 2.0,
        h - 2.0,
        1.5,
        P::SLOT_INSET,
        P::SLOT_FILL,
    );
    rounded(ui, v, i);
}

/// Scrollbar thumb: a raised rounded grip that brightens under the pointer.
pub fn scroll_thumb(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, hovered: bool) {
    let (top, mid) = if hovered {
        (P::BTN_HOVER_TOP, P::BTN_HOVER_MID)
    } else {
        (P::BTN_FACE_TOP, P::BTN_FACE_MID)
    };
    let (v, i) = draw::rounded_rect(x, y, w, h, 2.0, P::BTN_BORDER_ACTIVE);
    rounded(ui, v, i);
    let (v, i) = draw::rounded_rect_vgrad(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 1.5, top, mid);
    rounded(ui, v, i);
}
