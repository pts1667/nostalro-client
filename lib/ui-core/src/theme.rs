use crate::draw::{self, DrawCall, TextureRef};
use crate::frame::UiFrame;
use crate::rect::Rect;
use ragnarok_renderer::ui_renderer::UiVertex;

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Palette reproducing the light, glossy, blue-tinted `basic_interface` look for
/// every procedural widget and window. Values sampled from the real interface BMPs.
pub struct FallbackPalette;

impl FallbackPalette {
    pub const WIN_BODY: [f32; 4] = rgb(0xFA, 0xFB, 0xFF);
    pub const WIN_BODY_ALT: [f32; 4] = rgb(0xF2, 0xF2, 0xF2);
    pub const WIN_BORDER: [f32; 4] = rgb(0xC2, 0xC2, 0xC2);
    pub const WIN_BORDER_HI: [f32; 4] = rgb(0xFF, 0xFF, 0xFF);

    pub const TITLE_TOP: [f32; 4] = rgb(0x82, 0x94, 0xC8);
    pub const TITLE_BOT: [f32; 4] = rgb(0xD2, 0xEA, 0xFC);
    pub const TITLE_BASELINE: [f32; 4] = rgb(0x00, 0x00, 0x00);

    /// Two-tone gloss shared by the title bar, round system buttons and the
    /// scrollbar. Read bottom to top: pale blue darkens towards the middle, steps
    /// sharply to the darkest blue at 50% height, then lightens to the top edge.
    pub const GLOSS_UP_TOP: [f32; 4] = rgb(0xA4, 0xBF, 0xF2);
    pub const GLOSS_UP_BOT: [f32; 4] = rgb(0x4A, 0x6A, 0xBC);
    pub const GLOSS_LO_TOP: [f32; 4] = rgb(0x78, 0x9B, 0xE4);
    pub const GLOSS_LO_BOT: [f32; 4] = rgb(0xC8, 0xE0, 0xFB);
    pub const GLOSS_RIM: [f32; 4] = rgb(0x26, 0x42, 0x8E);
    pub const GLOSS_GLYPH: [f32; 4] = rgb(0xFF, 0xFF, 0xFF);

    /// Labelled buttons use the same two tones, lightened so dark text stays
    /// readable across the step at half height.
    pub const BTN_UP_TOP: [f32; 4] = rgb(0xF2, 0xF7, 0xFF);
    pub const BTN_UP_BOT: [f32; 4] = rgb(0xA6, 0xC0, 0xEE);
    pub const BTN_LO_TOP: [f32; 4] = rgb(0x8C, 0xAE, 0xE8);
    pub const BTN_LO_BOT: [f32; 4] = rgb(0xD6, 0xE7, 0xFC);

    pub const SLOT_FILL: [f32; 4] = rgb(0xFF, 0xFF, 0xFF);
    pub const SLOT_INSET: [f32; 4] = rgb(0xCF, 0xD6, 0xE5);

    pub const BTN_BORDER: [f32; 4] = rgb(0xCF, 0xC7, 0xCB);
    pub const BTN_BORDER_ACTIVE: [f32; 4] = rgb(0xAC, 0xA4, 0xA8);

    pub const BTN_FACE_TOP: [f32; 4] = rgb(0xED, 0xE8, 0xEA);
    pub const BTN_FACE_MID: [f32; 4] = rgb(0xE6, 0xE0, 0xE3);
    pub const BTN_FACE_BOT: [f32; 4] = rgb(0xF9, 0xF5, 0xF7);

    pub const BTN_HOVER_TOP: [f32; 4] = rgb(0xD0, 0xD7, 0xEA);
    pub const BTN_HOVER_MID: [f32; 4] = rgb(0xAE, 0xC0, 0xE6);
    pub const BTN_HOVER_BOT: [f32; 4] = rgb(0xD2, 0xDC, 0xF7);

    pub const BTN_PRESS_TOP: [f32; 4] = rgb(0xBF, 0xC9, 0xE6);
    pub const BTN_PRESS_MID: [f32; 4] = rgb(0x9A, 0xB0, 0xDD);
    pub const BTN_PRESS_BOT: [f32; 4] = rgb(0xC2, 0xCF, 0xF3);

    pub const BTN_INSET_SHADOW_TOP: [f32; 4] = rgb(0xA9, 0xA2, 0xA5);
    pub const BTN_INSET_SHADOW_BOT: [f32; 4] = rgb(0xD4, 0xCD, 0xD0);
    pub const BTN_INSET_SHADOW_BLUE_TOP: [f32; 4] = rgb(0x9E, 0xA6, 0xBE);
    pub const BTN_INSET_SHADOW_BLUE_BOT: [f32; 4] = rgb(0xC6, 0xCF, 0xE6);

    pub const SYS_BTN: [f32; 4] = rgb(0x3E, 0x6B, 0xC7);
    pub const SYS_BTN_HOVER: [f32; 4] = rgb(0x57, 0x96, 0xFE);
    pub const SYS_GLYPH: [f32; 4] = rgb(0x09, 0x23, 0x5A);

    pub const TEXT_ON_LIGHT: [f32; 4] = rgb(0x20, 0x20, 0x20);
}

pub const CORNER_RADIUS: f32 = 3.0;

fn push_white(ui: &mut UiFrame, verts: Vec<UiVertex>, indices: Vec<u32>) {
    ui.draw_calls.push(DrawCall {
        vertices: verts,
        indices,
        texture: TextureRef::White,
    });
}

/// 1px raised bevel: `hi` on top+left edges, `lo` on bottom+right. Edge segments
/// are inset by `radius` so they stay inside the rounded silhouette.
pub fn bevel(ui: &mut UiFrame, r: Rect, radius: f32, hi: [f32; 4], lo: [f32; 4]) {
    let rad = radius.max(0.0);
    let seg = |ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]| {
        if w > 0.0 && h > 0.0 {
            let (v, i) = draw::quad_vertices(x, y, w, h, c);
            push_white(ui, v.to_vec(), i.to_vec());
        }
    };
    seg(ui, r.x + rad, r.y, r.w - 2.0 * rad, 1.0, hi);
    seg(ui, r.x, r.y + rad, 1.0, r.h - 2.0 * rad, hi);
    seg(ui, r.x + rad, r.y + r.h - 1.0, r.w - 2.0 * rad, 1.0, lo);
    seg(ui, r.x + r.w - 1.0, r.y + rad, 1.0, r.h - 2.0 * rad, lo);
}

pub fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

/// Brightens a colour for the hovered state.
pub fn lift(c: [f32; 4], hovered: bool) -> [f32; 4] {
    if hovered {
        mix(c, [1.0, 1.0, 1.0, c[3]], 0.30)
    } else {
        c
    }
}

/// Half of a disc as a triangle fan, lerping `at_diameter` to `at_far` with the
/// distance from the horizontal diameter. `upper` picks the half above it.
fn half_disc(
    cx: f32,
    cy: f32,
    r: f32,
    upper: bool,
    at_diameter: [f32; 4],
    at_far: [f32; 4],
) -> (Vec<UiVertex>, Vec<u32>) {
    const SEGS: usize = 10;
    let sign = if upper { -1.0 } else { 1.0 };
    let vertex = |p: [f32; 2], color| UiVertex {
        position: p,
        tex_coord: [0.0, 0.0],
        color,
    };
    let mut verts = vec![vertex([cx, cy], at_diameter)];
    for k in 0..=SEGS {
        let a = std::f32::consts::PI * k as f32 / SEGS as f32;
        let (dx, dy) = (r * a.cos(), sign * r * a.sin());
        let t = (dy.abs() / r).clamp(0.0, 1.0);
        verts.push(vertex([cx + dx, cy + dy], mix(at_diameter, at_far, t)));
    }
    let mut indices = Vec::with_capacity(SEGS * 3);
    for k in 0..SEGS as u32 {
        indices.extend([0, 1 + k, 2 + k]);
    }
    (verts, indices)
}

/// Round glossy blue disc: a dark rim around the two-tone face, with a soft
/// highlight on the upper half.
pub fn gloss_disc(ui: &mut UiFrame, cx: f32, cy: f32, r: f32, hovered: bool) {
    for upper in [true, false] {
        let (v, i) = half_disc(cx, cy, r, upper, FallbackPalette::GLOSS_RIM, FallbackPalette::GLOSS_RIM);
        push_white(ui, v, i);
    }
    let r = r - 1.0;
    let (v, i) = half_disc(
        cx,
        cy,
        r,
        true,
        lift(FallbackPalette::GLOSS_UP_BOT, hovered),
        lift(FallbackPalette::GLOSS_UP_TOP, hovered),
    );
    push_white(ui, v, i);
    let (v, i) = half_disc(
        cx,
        cy,
        r,
        false,
        lift(FallbackPalette::GLOSS_LO_TOP, hovered),
        lift(FallbackPalette::GLOSS_LO_BOT, hovered),
    );
    push_white(ui, v, i);
    let (v, i) = draw::rounded_rect(
        cx - r * 0.55,
        cy - r * 0.85,
        r * 1.1,
        r * 0.45,
        r * 0.22,
        [1.0, 1.0, 1.0, 0.40],
    );
    push_white(ui, v, i);
}

/// Straight line segment of the given thickness, drawn as a rotated quad.
pub fn line(ui: &mut UiFrame, a: (f32, f32), b: (f32, f32), thickness: f32, color: [f32; 4]) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.0 {
        return;
    }
    let (nx, ny) = (-dy / len * thickness * 0.5, dx / len * thickness * 0.5);
    let vertex = |x: f32, y: f32| UiVertex {
        position: [x, y],
        tex_coord: [0.0, 0.0],
        color,
    };
    ui.draw_calls.push(DrawCall {
        vertices: vec![
            vertex(a.0 + nx, a.1 + ny),
            vertex(a.0 - nx, a.1 - ny),
            vertex(b.0 + nx, b.1 + ny),
            vertex(b.0 - nx, b.1 - ny),
        ],
        indices: vec![0, 1, 2, 2, 1, 3],
        texture: TextureRef::White,
    });
}

/// Glossy blue button: a dark rim around the two-tone face (light above, mid blue
/// below, meeting at half height), a top bevel line and a centred dark label.
/// Hover brightens the face; pressed darkens it.
pub fn fallback_button(ui: &mut UiFrame, r: Rect, hovered: bool, pressed: bool, label: &str) {
    let shade = |c: [f32; 4]| {
        let c = lift(c, hovered);
        if pressed {
            mix(c, FallbackPalette::GLOSS_RIM, 0.22)
        } else {
            c
        }
    };
    let (v, i) = draw::rounded_rect(r.x, r.y, r.w, r.h, CORNER_RADIUS, FallbackPalette::GLOSS_RIM);
    push_white(ui, v, i);

    let (fx, fy, fw, fh) = (r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
    let fr = (CORNER_RADIUS - 1.0).max(0.0);
    let upper = (fh * 0.5).floor();
    let (v, i) = draw::rounded_rect_corners_vgrad(
        fx,
        fy,
        fw,
        upper,
        [fr, fr, 0.0, 0.0],
        shade(FallbackPalette::BTN_UP_TOP),
        shade(FallbackPalette::BTN_UP_BOT),
    );
    push_white(ui, v, i);
    let (v, i) = draw::rounded_rect_corners_vgrad(
        fx,
        fy + upper,
        fw,
        fh - upper,
        [0.0, 0.0, fr, fr],
        shade(FallbackPalette::BTN_LO_TOP),
        shade(FallbackPalette::BTN_LO_BOT),
    );
    push_white(ui, v, i);
    let (v, i) = draw::quad_vertices(fx + fr, fy, fw - 2.0 * fr, 1.0, [1.0, 1.0, 1.0, 0.55]);
    push_white(ui, v.to_vec(), i.to_vec());

    if !label.is_empty() {
        let tw = ui.atlas.measure_text(label);
        let tx = r.x + (r.w - tw) / 2.0;
        let ty = r.y + r.h - (ui.atlas.line_height / 2.0);
        let (v, i) = draw::text_vertices(label, tx, ty, FallbackPalette::TEXT_ON_LIGHT, ui.atlas);
        if !v.is_empty() {
            ui.draw_calls.push(DrawCall {
                vertices: v,
                indices: i,
                texture: TextureRef::FontAtlas,
            });
        }
    }
}

/// Light rounded text field; border tints blue while focused.
pub fn fallback_text_input(ui: &mut UiFrame, r: Rect, has_focus: bool) {
    let border = if has_focus {
        FallbackPalette::TITLE_TOP
    } else {
        FallbackPalette::WIN_BORDER
    };
    fallback_panel(
        ui,
        r,
        CORNER_RADIUS,
        FallbackPalette::SLOT_FILL,
        FallbackPalette::SLOT_FILL,
        border,
    );
}

/// Rounded panel: a border-colored rounded rect with a 1px-inset gradient fill.
pub fn fallback_panel(
    ui: &mut UiFrame,
    r: Rect,
    radius: f32,
    fill_top: [f32; 4],
    fill_bot: [f32; 4],
    border: [f32; 4],
) {
    let (v, i) = draw::rounded_rect(r.x, r.y, r.w, r.h, radius, border);
    push_white(ui, v, i);
    let (v, i) = draw::rounded_rect_vgrad(
        r.x + 1.0,
        r.y + 1.0,
        r.w - 2.0,
        r.h - 2.0,
        (radius - 1.0).max(0.0),
        fill_top,
        fill_bot,
    );
    push_white(ui, v, i);
}

/// Rounded panel with a concave glossy fill: `top`→`mid` over the upper half,
/// `mid`→`bot` over the lower half, ringed by a `border`-colored rounded rect.
pub fn fallback_glossy_panel(
    ui: &mut UiFrame,
    r: Rect,
    radius: f32,
    top: [f32; 4],
    mid: [f32; 4],
    bot: [f32; 4],
    border: [f32; 4],
) {
    let (v, i) = draw::rounded_rect(r.x, r.y, r.w, r.h, radius, border);
    push_white(ui, v, i);
    let ir = (radius - 1.0).max(0.0);
    let (x, y, w, h) = (r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
    let half = (h * 0.5).round();
    let (v, i) = draw::rounded_rect_corners_vgrad(x, y, w, half, [ir, ir, 0.0, 0.0], top, mid);
    push_white(ui, v, i);
    let (v, i) =
        draw::rounded_rect_corners_vgrad(x, y + half, w, h - half, [0.0, 0.0, ir, ir], mid, bot);
    push_white(ui, v, i);
}
