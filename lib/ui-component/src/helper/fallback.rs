use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::UiFrame;
use ragnarok_ui::rect::Rect;
use ragnarok_ui::theme::{CORNER_RADIUS, FallbackPalette as P, gloss_disc, lift, line};

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
/// top-left system button. Two tones meet at half height: the upper half runs
/// light blue to dark blue, the lower half starts mid blue and ends pale.
pub fn titlebar(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let body = h - 1.0;
    let upper = (body * 0.5).floor();
    let (v, i) = draw::rounded_rect_corners_vgrad(
        x,
        y,
        w,
        upper,
        [CORNER_RADIUS, CORNER_RADIUS, 0.0, 0.0],
        P::GLOSS_UP_TOP,
        P::GLOSS_UP_BOT,
    );
    rounded(ui, v, i);
    let (v, i) =
        draw::quad_vertices_vgrad(x, y + upper, w, body - upper, P::GLOSS_LO_TOP, P::GLOSS_LO_BOT);
    ui.draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
    // Bevel: a faint light line along the top edge inside the rounded corners.
    fill(ui, x + CORNER_RADIUS, y + 1.0, w - 2.0 * CORNER_RADIUS, 1.0, [1.0, 1.0, 1.0, 0.45]);
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

/// White glyph with a dark outline, centred on `(cx, cy)`. `_`, `x` and `+` are
/// drawn from lines; anything else falls back to text.
fn sys_glyph(ui: &mut UiFrame, ch: char, cx: f32, cy: f32, size: f32) {
    let k = size * 0.2;
    let strokes: Vec<((f32, f32), (f32, f32))> = match ch {
        '_' => vec![((cx - k * 1.3, cy + k * 0.9), (cx + k * 1.3, cy + k * 0.9))],
        'x' => vec![
            ((cx - k, cy - k), (cx + k, cy + k)),
            ((cx - k, cy + k), (cx + k, cy - k)),
        ],
        '+' => vec![
            ((cx - k * 1.3, cy), (cx + k * 1.3, cy)),
            ((cx, cy - k * 1.3), (cx, cy + k * 1.3)),
        ],
        _ => {
            let s = ch.to_string();
            let tw = ui.atlas.measure_text(&s);
            let ty = cy + ui.atlas.line_height / 4.0;
            let (tv, ti) = draw::text_vertices(&s, cx - tw / 2.0, ty, P::GLOSS_GLYPH, ui.atlas);
            if !tv.is_empty() {
                ui.draw_calls.push(DrawCall {
                    vertices: tv,
                    indices: ti,
                    texture: TextureRef::FontAtlas,
                });
            }
            return;
        }
    };
    for (color, thickness) in [(P::GLOSS_RIM, 2.2), (P::GLOSS_GLYPH, 1.1)] {
        for (a, b) in &strokes {
            line(ui, *a, *b, thickness, color);
        }
    }
}

/// Round glossy blue system button (close, minimise) with a white glyph. With no
/// glyph it is the plain disc, as the title bar's menu button is.
pub fn sys_button(ui: &mut UiFrame, x: f32, y: f32, size: f32, hovered: bool, glyph: Option<char>) {
    let (cx, cy) = (x + size / 2.0, y + size / 2.0);
    gloss_disc(ui, cx, cy, size / 2.0, hovered);
    if let Some(ch) = glyph {
        sys_glyph(ui, ch, cx, cy, size);
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

/// Scrollbar arrow button: a round glossy disc with a white triangle drawn from
/// quads, so it does not depend on a glyph being in the font.
pub fn scroll_button(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, up: bool, hovered: bool) {
    // Slightly under half so no vertex lands exactly on a pixel centre.
    let r = w.min(h) / 2.0 - 0.7;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    gloss_disc(ui, cx, cy, r, hovered);

    let (tip, base) = if up { (-2.5, 1.5) } else { (2.5, -1.5) };
    for (color, grow) in [(P::GLOSS_RIM, 1.0), (P::GLOSS_GLYPH, 0.0)] {
        let sign = if up { -1.0 } else { 1.0 };
        let pts = [
            (cx, cy + tip + sign * grow * 1.2),
            (cx - 3.0 - grow, cy + base - sign * grow * 0.6),
            (cx + 3.0 + grow, cy + base - sign * grow * 0.6),
        ];
        let (v, i) = draw::triangle_vertices(pts, color);
        rounded(ui, v.to_vec(), i.to_vec());
    }
}

/// Recessed track behind a scrollbar thumb: a narrow grey capsule.
pub fn scroll_track(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    let (tx, tw) = (x + 3.0, w - 6.0);
    let (v, i) = draw::rounded_rect(tx, y, tw, h, tw / 2.0, [0.60, 0.64, 0.72, 1.0]);
    rounded(ui, v, i);
    let (v, i) = draw::rounded_rect_vgrad(
        tx + 1.0,
        y + 1.0,
        tw - 2.0,
        h - 2.0,
        (tw - 2.0) / 2.0,
        [0.80, 0.83, 0.89, 1.0],
        [0.93, 0.94, 0.97, 1.0],
    );
    rounded(ui, v, i);
}

/// Scrollbar thumb: a glossy blue capsule with a light streak down its left side.
pub fn scroll_thumb(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, hovered: bool) {
    let (v, i) = draw::rounded_rect(x, y, w, h, w / 2.0, P::GLOSS_RIM);
    rounded(ui, v, i);
    let (v, i) = draw::rounded_rect_vgrad(
        x + 1.0,
        y + 1.0,
        w - 2.0,
        h - 2.0,
        (w - 2.0) / 2.0,
        lift(P::GLOSS_UP_TOP, hovered),
        lift(P::GLOSS_UP_BOT, hovered),
    );
    rounded(ui, v, i);
    let streak = (w * 0.28).max(1.5);
    let (v, i) = draw::rounded_rect(
        x + 2.0,
        y + 3.0,
        streak,
        (h - 6.0).max(1.0),
        streak / 2.0,
        [1.0, 1.0, 1.0, 0.40],
    );
    rounded(ui, v, i);
}
