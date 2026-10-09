use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::UiFrame;
use ragnarok_ui::rect::Rect;

fn push_quad(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    let (v, i) = draw::quad_vertices(x, y, w, h, color);
    ui.draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::White,
    });
}

pub fn draw_textured_quad(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, path: &str) {
    let (v, i) = draw::quad_vertices(x, y, w, h, [1.0; 4]);
    ui.draw_calls.push(DrawCall {
        vertices: v.to_vec(),
        indices: i.to_vec(),
        texture: TextureRef::Named(path.to_string()),
    });
}

/// Small rounded system button (close, minimize), drawn procedurally.
pub fn draw_sys_button(
    ui: &mut UiFrame,
    rect: Rect,
    size: (f32, f32),
    hovered: bool,
    fallback_char: Option<char>,
) {
    crate::helper::fallback::sys_button(ui, rect.x, rect.y, size.0, hovered, fallback_char);
}



/// HP/SP gauge: a rounded bar filled to `fill_pct`, red or blue.
pub fn draw_gauge(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, fill_pct: f32, is_red: bool) {
    crate::helper::fallback::gauge(ui, x, y, w, h, fill_pct.clamp(0.0, 1.0), is_red);
}

/// Body text colour. Dark on the light panels in both looks.
pub fn text_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

/// Dark navy blue used for field labels (Name/HP/SP/stat names) in info windows.
pub fn label_color() -> [f32; 4] {
    [0.14, 0.20, 0.38, 1.0]
}

pub fn draw_hline(ui: &mut UiFrame, x: f32, y: f32, w: f32) {
    push_quad(ui, x, y, w, 1.0, [0.7, 0.7, 0.72, 1.0]);
}

pub const EXP_BAR_FILL: [f32; 4] = [0.26, 0.38, 0.65, 1.0];

/// Thin EXP-style bar (grey frame, white well, blue fill) matching the character
/// info window's exp gauge.
pub fn draw_exp_bar(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, fill_pct: f32) {
    draw_value_bar(ui, x, y, w, h, fill_pct, EXP_BAR_FILL);
}

/// [`draw_exp_bar`] with the fill colour chosen by the caller.
pub fn draw_value_bar(
    ui: &mut UiFrame,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill_pct: f32,
    fill: [f32; 4],
) {
    // Grey frame with a white well.
    use ragnarok_ui::theme::FallbackPalette as P;
    push_quad(ui, x, y, w + 2.0, h + 2.0, P::WIN_BORDER);
    push_quad(ui, x + 1.0, y + 1.0, w, h, P::SLOT_FILL);
    if fill_pct > 0.0 {
        let fw = (w * fill_pct.clamp(0.0, 1.0)).floor();
        push_quad(ui, x + 1.0, y + 1.0, fw, h, fill);
    }
}

pub fn draw_titlebar(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    crate::helper::fallback::titlebar(ui, x, y, w, h);
}

pub fn draw_container(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    crate::helper::fallback::container(ui, x, y, w, h);
}

pub fn draw_footer(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32) {
    crate::helper::fallback::footer(ui, x, y, w, h);
}
