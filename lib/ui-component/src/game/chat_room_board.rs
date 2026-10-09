use crate::helper::dialog_container::DialogContainer;
use crate::helper::head_board::{self, BoardIcon};
use ragnarok_renderer::font_atlas::FontAtlas;
use ragnarok_ui::draw::DrawCall;

pub use crate::helper::head_board::board_rect;

fn board_icon(atype: u8) -> BoardIcon {
    if atype == 0 {
        BoardIcon::ChatClose
    } else {
        BoardIcon::ChatOpen
    }
}

fn count_suffix(atype: u8, cur: i16, max: i16) -> String {
    if atype == 3 {
        String::new()
    } else {
        format!(" ({cur}/{max})")
    }
}

/// Draw the room board (frame + open/close icon + title/count) into `draw_calls`.
/// The title is pixel-truncated to the board width, keeping the count suffix
/// fully visible.
#[allow(clippy::too_many_arguments)]
pub fn draw_board(
    draw_calls: &mut Vec<DrawCall>,
    container: &DialogContainer,
    atlas: &FontAtlas,
    anchor_x: f32,
    anchor_y: f32,
    head_offset: f32,
    atype: u8,
    title: &str,
    cur: i16,
    max: i16,
) {
    let suffix = count_suffix(atype, cur, max);
    let title_width = (head_board::label_max_width() - atlas.measure_text(&suffix)).max(0.0);
    let title = head_board::truncate_to_width(title, title_width, atlas);
    head_board::draw_board(
        draw_calls,
        container,
        atlas,
        anchor_x,
        anchor_y,
        head_offset,
        board_icon(atype),
        &format!("{title}{suffix}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_and_count_suffix_follow_room_type() {
        assert_eq!(board_icon(2), BoardIcon::ChatOpen);
        assert_eq!(board_icon(0), BoardIcon::ChatClose);
        assert_eq!(count_suffix(2, 3, 20), " (3/20)");
        assert_eq!(count_suffix(3, 3, 20), "");
    }
}
