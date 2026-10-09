use crate::helper::dialog_container::DialogContainer;
use crate::helper::fallback;
use ragnarok_renderer::font_atlas::FontAtlas;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};

const PAD: f32 = 4.0;
const ICON: f32 = 24.0;
/// Fixed board width; long labels are clipped by the caller.
pub const BOARD_W: f32 = 140.0;

/// Screen-space `[x0, y0, x1, y1]` of a board centered above a head anchor.
pub fn board_rect(anchor_x: f32, anchor_y: f32, head_offset: f32) -> [f32; 4] {
    let box_h = PAD + ICON + PAD;
    let box_x = anchor_x - BOARD_W / 2.0;
    let box_y = anchor_y - head_offset - 5.0 - box_h;
    [box_x, box_y, box_x + BOARD_W, box_y + box_h]
}

/// Pixel width available to the label between the icon and the right edge.
pub fn label_max_width() -> f32 {
    BOARD_W - ICON - 3.0 * PAD
}

/// The icon on the left of a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardIcon {
    /// Vendor's shop: striped awning over a shop front.
    Shop,
    /// Chat room open to visitors: speech bubble with dots.
    ChatOpen,
    /// Chat room closed: grey speech bubble with a bar.
    ChatClose,
}

/// Procedural icon, drawn in the `ICON` square at `(x, y)`.
fn draw_icon(draw_calls: &mut Vec<DrawCall>, icon: BoardIcon, x: f32, y: f32) {
    const RED: [f32; 4] = [0.80, 0.18, 0.18, 1.0];
    const WHITE: [f32; 4] = [0.98, 0.98, 0.98, 1.0];
    const WOOD: [f32; 4] = [0.55, 0.38, 0.22, 1.0];
    const DOOR: [f32; 4] = [0.25, 0.16, 0.09, 1.0];
    const INK: [f32; 4] = [0.20, 0.20, 0.25, 1.0];
    const BLUE: [f32; 4] = [0.36, 0.56, 0.86, 1.0];
    const GREY: [f32; 4] = [0.60, 0.60, 0.64, 1.0];
    match icon {
        BoardIcon::Shop => {
            for n in 0..4 {
                let color = if n % 2 == 0 { RED } else { WHITE };
                fallback::fill_into(draw_calls, x + 2.0 + n as f32 * 5.0, y + 3.0, 5.0, 6.0, color);
            }
            fallback::fill_into(draw_calls, x + 4.0, y + 9.0, 16.0, 13.0, WOOD);
            fallback::fill_into(draw_calls, x + 10.0, y + 14.0, 4.0, 8.0, DOOR);
        }
        BoardIcon::ChatOpen | BoardIcon::ChatClose => {
            let open = icon == BoardIcon::ChatOpen;
            let body = if open { BLUE } else { GREY };
            let (v, i) = draw::rounded_rect(x + 2.0, y + 3.0, 20.0, 14.0, 4.0, body);
            draw_calls.push(DrawCall {
                vertices: v,
                indices: i,
                texture: TextureRef::White,
            });
            fallback::triangle_into(
                draw_calls,
                [(x + 6.0, y + 16.0), (x + 12.0, y + 16.0), (x + 5.0, y + 22.0)],
                body,
            );
            if open {
                for dot in 0..3 {
                    fallback::fill_into(draw_calls, x + 7.0 + dot as f32 * 4.0, y + 9.0, 2.0, 2.0, WHITE);
                }
            } else {
                fallback::fill_into(draw_calls, x + 6.0, y + 9.0, 12.0, 2.0, INK);
            }
        }
    }
}

/// Draw a labeled board above a head: frame + a left icon + the `label`
/// as-is (callers pre-clip/compose it).
pub fn draw_board(
    draw_calls: &mut Vec<DrawCall>,
    container: &DialogContainer,
    atlas: &FontAtlas,
    anchor_x: f32,
    anchor_y: f32,
    head_offset: f32,
    icon: BoardIcon,
    label: &str,
) {
    let [box_x, box_y, box_x1, box_y1] = board_rect(anchor_x, anchor_y, head_offset);
    let box_w = box_x1 - box_x;
    let box_h = box_y1 - box_y;

    container.draw(draw_calls, box_x, box_y, box_w, box_h, [1.0; 4]);

    let icon_x = box_x + PAD;
    let icon_y = box_y + (box_h - ICON) / 2.0;
    draw_icon(draw_calls, icon, icon_x, icon_y);

    let text_x = box_x + PAD + ICON + PAD;
    let text_y = box_y + (box_h + atlas.line_height) / 2.0;
    let (verts, indices) =
        draw::text_vertices(label, text_x, text_y, container.text_color(), atlas);
    if !verts.is_empty() {
        draw_calls.push(DrawCall {
            vertices: verts,
            indices,
            texture: TextureRef::FontAtlas,
        });
    }
}

/// Clip `text` to `max_w` pixels, appending `…` when it doesn't fit.
pub fn truncate_to_width(text: &str, max_w: f32, atlas: &FontAtlas) -> String {
    if atlas.measure_text(text) <= max_w {
        return text.to_string();
    }
    let ellipsis = "…";
    let ellipsis_w = atlas.measure_text(ellipsis);
    let mut out = String::new();
    for ch in text.chars() {
        let mut trial = out.clone();
        trial.push(ch);
        if atlas.measure_text(&trial) + ellipsis_w > max_w {
            break;
        }
        out.push(ch);
    }
    out.push_str(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_sits_centered_above_owner_head() {
        let [x0, y0, x1, y1] = board_rect(100.0, 200.0, 50.0);
        assert!(((x0 + x1) / 2.0 - 100.0).abs() < f32::EPSILON);
        assert!((x1 - x0 - BOARD_W).abs() < f32::EPSILON);
        let _ = y0;
        assert!(y1 < 200.0);
    }
}
