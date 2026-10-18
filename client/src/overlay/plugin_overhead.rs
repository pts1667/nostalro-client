//! Notifications plugins draw above units (rust-ro `docs/plugin-ui.md`): an image, a caption under it, or both,
//! moved by the overhead's motions until its duration ends.

use std::collections::HashMap;
use std::time::Instant;

use plugin_ui::Overhead;
use ragnarok_game::cursor::RenderEntry;
use ragnarok_renderer::font_atlas::FontAtlas;
use ragnarok_renderer::{UiDrawCall, UiTextureRef};
use ragnarok_ui_component::game::plugin_window::{parse_rgb, texture_path};

use crate::App;

/// Older notifications give way past this many.
const MAX_OVERHEADS: usize = 32;
const OUTLINE: [(f32, f32); 8] = [
    (-1.0, -1.0),
    (0.0, -1.0),
    (1.0, -1.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, 1.0),
    (0.0, 1.0),
    (1.0, 1.0),
];
const TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const LIFT: f32 = 6.0;
/// Between overheads of one unit shown at the same time, such as a base and a job level up.
const STACK_GAP: f32 = 4.0;

pub struct PluginOverhead {
    /// The unit's id as the server names it: an account id for players.
    pub unit_id: u32,
    pub overhead: Overhead,
    pub shown_at: Instant,
}

/// Adds `overhead`, dropping the ones that ended and the oldest past [`MAX_OVERHEADS`].
pub fn push(overheads: &mut Vec<PluginOverhead>, unit_id: u32, overhead: Overhead, now: Instant) {
    overheads.retain(|shown| !shown.overhead.finished(elapsed_ms(shown, now)));
    if overheads.len() >= MAX_OVERHEADS {
        overheads.remove(0);
    }
    overheads.push(PluginOverhead {
        unit_id,
        overhead,
        shown_at: now,
    });
}

fn elapsed_ms(shown: &PluginOverhead, now: Instant) -> u32 {
    u32::try_from(now.saturating_duration_since(shown.shown_at).as_millis()).unwrap_or(u32::MAX)
}

impl App {
    pub(super) fn build_plugin_overheads(
        &self,
        render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let now = Instant::now();
        let mut stacked: HashMap<u32, f32> = HashMap::new();
        for shown in &self.game.plugin_overheads {
            let elapsed = elapsed_ms(shown, now);
            if shown.overhead.finished(elapsed) {
                continue;
            }
            let key = self.game.world.entities.resolve_key(shown.unit_id);
            let Some(entry) = render_list.iter().find(|entry| entry.id == key) else {
                continue;
            };
            let below = stacked.entry(key).or_insert(0.0);
            let top = entry.screen_anchor[1] - entry.head_offset - LIFT - *below;
            *below += resting_height(&shown.overhead, &renderer.font_atlas) + STACK_GAP;
            calls.extend(overhead_calls(
                &shown.overhead,
                elapsed,
                entry.screen_anchor[0],
                top,
                &renderer.font_atlas,
            ));
        }
    }
}

/// Image and caption, before motions; what a later overhead of the same unit stacks on.
fn resting_height(overhead: &Overhead, atlas: &FontAtlas) -> f32 {
    let image = if overhead.image.is_some() {
        f32::from(overhead.height)
    } else {
        0.0
    };
    let text = if overhead.text.is_some() {
        atlas.line_height
    } else {
        0.0
    };
    image + text + f32::from(overhead.offset_y.max(0))
}

/// The overhead `elapsed_ms` after it appeared, its bottom resting `bottom` pixels down the screen.
fn overhead_calls(
    overhead: &Overhead,
    elapsed_ms: u32,
    center_x: f32,
    bottom: f32,
    atlas: &FontAtlas,
) -> Vec<UiDrawCall> {
    let mut calls = Vec::new();
    let transform = overhead.transform(elapsed_ms);
    if transform.alpha <= 0.0 || transform.scale <= 0.01 {
        return calls;
    }
    let scale = transform.scale;
    let center_x = center_x + transform.dx;
    let bottom = bottom - f32::from(overhead.offset_y) + transform.dy;
    let text_h = if overhead.text.is_some() {
        atlas.line_height * scale
    } else {
        0.0
    };

    if let Some(frame) = overhead
        .image
        .as_ref()
        .and_then(|image| image.frame_at(elapsed_ms as f32 / 1000.0))
    {
        let (w, h) = (
            f32::from(overhead.width) * scale,
            f32::from(overhead.height) * scale,
        );
        let (vertices, indices) = ragnarok_ui::draw::quad_vertices(
            center_x - w / 2.0,
            bottom - text_h - h,
            w,
            h,
            [1.0, 1.0, 1.0, transform.alpha],
        );
        calls.push(UiDrawCall {
            vertices: vertices.to_vec(),
            indices: indices.to_vec(),
            texture: UiTextureRef::Named(texture_path(frame)),
        });
    }

    if let Some(text) = &overhead.text {
        let mut color = text
            .color
            .as_deref()
            .and_then(parse_rgb)
            .unwrap_or(TEXT_COLOR);
        color[3] *= transform.alpha;
        let x = center_x - atlas.measure_text(&text.text) * scale / 2.0;
        let baseline = bottom - text_h + (text_h + atlas.ascent * scale) / 2.0;
        let shadow = [0.0, 0.0, 0.0, color[3]];
        let step = scale.max(1.0);
        for (dx, dy) in OUTLINE {
            push_text(
                &mut calls,
                &text.text,
                x + dx * step,
                baseline + dy * step,
                shadow,
                atlas,
                scale,
            );
        }
        push_text(&mut calls, &text.text, x, baseline, color, atlas, scale);
    }
    calls
}

fn push_text(
    calls: &mut Vec<UiDrawCall>,
    text: &str,
    x: f32,
    y: f32,
    color: [f32; 4],
    atlas: &FontAtlas,
    scale: f32,
) {
    let (vertices, indices) =
        ragnarok_ui::draw::text_vertices_scaled(text, x, y, color, atlas, scale);
    if !vertices.is_empty() {
        calls.push(UiDrawCall {
            vertices,
            indices,
            texture: UiTextureRef::FontAtlas,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use plugin_ui::{Easing, ImageRef};

    use super::*;

    fn level_up() -> Overhead {
        Overhead::image(ImageRef::texture("plugin/ui_shop/star.png"), 48, 48, 2000)
            .caption("LEVEL 2!", Some("ffd040"))
            .rise(40.0, 1000)
            .easing(Easing::Linear)
            .fade_out(500)
    }

    fn image_rect(calls: &[UiDrawCall]) -> (f32, f32, f32) {
        let call = calls
            .iter()
            .find(|call| matches!(call.texture, UiTextureRef::Named(_)))
            .expect("the image");
        let ys: Vec<f32> = call
            .vertices
            .iter()
            .map(|vertex| vertex.position[1])
            .collect();
        let alpha = call.vertices[0].color[3];
        (
            ys.iter().cloned().fold(f32::MAX, f32::min),
            ys.iter().cloned().fold(f32::MIN, f32::max),
            alpha,
        )
    }

    #[test]
    fn the_image_rises_above_its_caption_and_fades_at_the_end() {
        let atlas = ragnarok_ui::test_support::atlas();
        let start = overhead_calls(&level_up(), 0, 100.0, 300.0, atlas);
        let (top, bottom, alpha) = image_rect(&start);
        assert_eq!(
            bottom,
            300.0 - atlas.line_height,
            "the caption sits under the image"
        );
        assert_eq!(bottom - top, 48.0);
        assert_eq!(alpha, 1.0);
        assert!(
            start
                .iter()
                .any(|call| matches!(call.texture, UiTextureRef::FontAtlas))
        );
        assert!(
            matches!(&start[0].texture, UiTextureRef::Named(path) if path == "data/plugin/ui_shop/star.png")
        );

        let (_, risen, _) = image_rect(&overhead_calls(&level_up(), 1000, 100.0, 300.0, atlas));
        assert_eq!(risen, bottom - 40.0);
        let (_, _, fading) = image_rect(&overhead_calls(&level_up(), 1750, 100.0, 300.0, atlas));
        assert!(fading > 0.0 && fading < 1.0);
    }

    #[test]
    fn a_stacked_overhead_rests_above_the_image_and_caption_below_it() {
        let atlas = ragnarok_ui::test_support::atlas();
        assert_eq!(resting_height(&level_up(), atlas), 48.0 + atlas.line_height);
        assert_eq!(
            resting_height(&Overhead::text("Hi", None, 500).offset(-8), atlas),
            atlas.line_height
        );
    }

    #[test]
    fn ended_overheads_make_room_and_the_oldest_gives_way() {
        let start = Instant::now();
        let mut overheads = Vec::new();
        push(&mut overheads, 1, Overhead::text("old", None, 100), start);
        push(
            &mut overheads,
            2,
            Overhead::text("new", None, 5000),
            start + Duration::from_millis(200),
        );
        assert_eq!(
            overheads
                .iter()
                .map(|shown| shown.unit_id)
                .collect::<Vec<_>>(),
            [2],
            "the first one ended"
        );
        for unit in 0..MAX_OVERHEADS as u32 {
            push(
                &mut overheads,
                10 + unit,
                Overhead::text("x", None, 5000),
                start + Duration::from_millis(300),
            );
        }
        assert_eq!(overheads.len(), MAX_OVERHEADS);
        assert_eq!(overheads[0].unit_id, 10);
    }
}
