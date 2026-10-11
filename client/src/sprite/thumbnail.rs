//! A still picture of a sprite for windows: the first frame of its first
//! action (idle, facing the camera), with every layer of that frame stacked.

use ragnarok_formats::act::{ActFile, Motion};
use ragnarok_formats::spr::{RgbaImageData, SprFile};

/// Longest side a thumbnail may have; sprites larger than this are cropped to it.
const MAX_SIDE: i32 = 512;

pub(crate) struct Thumbnail {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Builds the thumbnail of a sprite from the bytes of its `.spr` and `.act`.
pub(crate) fn from_files(spr_bytes: &[u8], act_bytes: &[u8]) -> Option<Thumbnail> {
    let spr = SprFile::parse(spr_bytes).ok()?;
    let act = ActFile::parse(act_bytes).ok()?;
    let (images, indexed_count) = match spr.palette {
        Some(_) => spr.to_rgba_images(),
        // Without a palette only true-colour images can be read.
        None if spr.indexed_sprites.is_empty() => {
            (spr.rgba_sprites.iter().map(|s| s.swizzle_pixel_bytes()).collect(), 0)
        }
        None => return None,
    };
    let motion = act.actions.first()?.motions.first()?;
    compose(&images, indexed_count, motion)
}

struct Placed<'a> {
    image: &'a RgbaImageData,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    mirror: bool,
    tint: [f32; 4],
}

fn compose(images: &[RgbaImageData], indexed_count: usize, motion: &Motion) -> Option<Thumbnail> {
    let placed: Vec<Placed> = motion
        .clips
        .iter()
        .filter(|clip| clip.sprite_index >= 0)
        .filter_map(|clip| {
            let index = if clip.sprite_type == 0 {
                clip.sprite_index as usize
            } else {
                indexed_count + clip.sprite_index as usize
            };
            let image = images.get(index)?;
            let zoom = |z: f32| if z.abs() > f32::EPSILON { z.abs() } else { 1.0 };
            let width = (image.width as f32 * zoom(clip.zoom_x)).round() as i32;
            let height = (image.height as f32 * zoom(clip.zoom_y)).round() as i32;
            if width <= 0 || height <= 0 {
                return None;
            }
            Some(Placed {
                image,
                // The layer is centred on its offset.
                left: clip.x - width / 2,
                top: clip.y - height / 2,
                width,
                height,
                mirror: clip.mirror & 1 == 1,
                tint: clip.color.map(|c| f32::from(c) / 255.0),
            })
        })
        .collect();
    if placed.is_empty() {
        return None;
    }

    let min_x = placed.iter().map(|p| p.left).min()?;
    let min_y = placed.iter().map(|p| p.top).min()?;
    let max_x = placed.iter().map(|p| p.left + p.width).max()?;
    let max_y = placed.iter().map(|p| p.top + p.height).max()?;
    let canvas_w = (max_x - min_x).min(MAX_SIDE);
    let canvas_h = (max_y - min_y).min(MAX_SIDE);
    let mut canvas = vec![0f32; (canvas_w * canvas_h * 4) as usize];

    for layer in &placed {
        for dy in 0..layer.height {
            let y = layer.top - min_y + dy;
            if !(0..canvas_h).contains(&y) {
                continue;
            }
            let sy = (dy as i64 * i64::from(layer.image.height) / i64::from(layer.height)) as usize;
            for dx in 0..layer.width {
                let x = layer.left - min_x + dx;
                if !(0..canvas_w).contains(&x) {
                    continue;
                }
                let along = if layer.mirror { layer.width - 1 - dx } else { dx };
                let sx = (along as i64 * i64::from(layer.image.width) / i64::from(layer.width)) as usize;
                let at = (sy * layer.image.width as usize + sx) * 4;
                let src = &layer.image.data[at..at + 4];
                let alpha = f32::from(src[3]) / 255.0 * layer.tint[3];
                if alpha <= 0.0 {
                    continue;
                }
                let out = ((y * canvas_w + x) * 4) as usize;
                let below = canvas[out + 3];
                let total = alpha + below * (1.0 - alpha);
                for channel in 0..3 {
                    let colour = f32::from(src[channel]) / 255.0 * layer.tint[channel];
                    canvas[out + channel] =
                        (colour * alpha + canvas[out + channel] * below * (1.0 - alpha)) / total;
                }
                canvas[out + 3] = total;
            }
        }
    }

    crop_to_content(&canvas, canvas_w, canvas_h)
}

/// Drops the transparent border and converts to bytes.
fn crop_to_content(canvas: &[f32], width: i32, height: i32) -> Option<Thumbnail> {
    let opaque = |x: i32, y: i32| canvas[((y * width + x) * 4 + 3) as usize] > 0.0;
    let xs: Vec<i32> = (0..width).filter(|&x| (0..height).any(|y| opaque(x, y))).collect();
    let ys: Vec<i32> = (0..height).filter(|&y| (0..width).any(|x| opaque(x, y))).collect();
    let (&left, &right) = (xs.first()?, xs.last()?);
    let (&top, &bottom) = (ys.first()?, ys.last()?);
    let (w, h) = (right - left + 1, bottom - top + 1);

    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in top..=bottom {
        for x in left..=right {
            let at = ((y * width + x) * 4) as usize;
            rgba.extend(canvas[at..at + 4].iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    Some(Thumbnail { rgba, width: w as u32, height: h as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_formats::act::SpriteFrame;

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImageData {
        RgbaImageData { width, height, data: rgba.repeat((width * height) as usize) }
    }

    fn clip(x: i32, y: i32, sprite_index: i32) -> SpriteFrame {
        SpriteFrame {
            x,
            y,
            sprite_index,
            mirror: 0,
            color: [255; 4],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 0,
            width: None,
            height: None,
        }
    }

    fn motion(clips: Vec<SpriteFrame>) -> Motion {
        Motion { range1: [0; 4], range2: [0; 4], clips, event_id: -1, attach_points: Vec::new() }
    }

    fn pixel(t: &Thumbnail, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * t.width + x) * 4) as usize;
        t.rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn a_single_layer_is_cropped_to_its_pixels() {
        let images = [solid(4, 2, [255, 0, 0, 255])];
        let t = compose(&images, 1, &motion(vec![clip(10, -3, 0)])).unwrap();
        assert_eq!((t.width, t.height), (4, 2));
        assert_eq!(pixel(&t, 3, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn layers_stack_in_order_around_their_offsets() {
        let images = [solid(4, 4, [255, 0, 0, 255]), solid(2, 2, [0, 0, 255, 255])];
        // The small layer sits to the right of the big one's centre.
        let t = compose(&images, 2, &motion(vec![clip(0, 0, 0), clip(2, 0, 1)])).unwrap();
        assert_eq!(t.width, 5);
        assert_eq!(pixel(&t, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&t, 4, 1), [0, 0, 255, 255], "the later layer is on top");
    }

    #[test]
    fn transparent_pixels_do_not_count_toward_the_crop() {
        let mut image = solid(4, 4, [0, 0, 0, 0]);
        image.data[(1 * 4 + 2) * 4..(1 * 4 + 2) * 4 + 4].copy_from_slice(&[9, 9, 9, 255]);
        let t = compose(&[image], 1, &motion(vec![clip(0, 0, 0)])).unwrap();
        assert_eq!((t.width, t.height), (1, 1));
        assert_eq!(pixel(&t, 0, 0), [9, 9, 9, 255]);
    }

    #[test]
    fn mirror_flips_the_layer_sideways() {
        let mut image = solid(2, 1, [0, 0, 0, 255]);
        image.data[0..4].copy_from_slice(&[255, 0, 0, 255]);
        let mut flipped = clip(0, 0, 0);
        flipped.mirror = 1;
        let t = compose(&[image], 1, &motion(vec![flipped])).unwrap();
        assert_eq!(pixel(&t, 1, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn rgba_layers_index_after_the_palette_ones() {
        let images = [solid(1, 1, [1, 1, 1, 255]), solid(1, 1, [200, 100, 50, 255])];
        let mut layer = clip(0, 0, 0);
        layer.sprite_type = 1;
        let t = compose(&images, 1, &motion(vec![layer])).unwrap();
        assert_eq!(pixel(&t, 0, 0), [200, 100, 50, 255]);
    }

    #[test]
    fn nothing_drawable_gives_none() {
        let images = [solid(2, 2, [0, 0, 0, 0])];
        assert!(compose(&images, 1, &motion(vec![clip(0, 0, 0)])).is_none());
        assert!(compose(&images, 1, &motion(vec![clip(0, 0, -1)])).is_none());
        assert!(compose(&images, 1, &motion(Vec::new())).is_none());
    }

    /// Dumps thumbnails of real sprites to `THUMB_OUT` as `<name>.rgba` with the
    /// size in the first 8 bytes. Needs `THUMB_GRF`; run with `--ignored`.
    #[test]
    #[ignore]
    fn dump_real_sprites() {
        let grf_path = std::env::var("THUMB_GRF").expect("THUMB_GRF");
        let out = std::env::var("THUMB_OUT").expect("THUMB_OUT");
        let grf = ragnarok_formats::grf::GrfArchive::open(std::path::Path::new(&grf_path)).unwrap();
        for name in ["poring", "lunatic", "baphomet", "drops"] {
            let base = ragnarok_resources::sprite::monster::of(name);
            let spr = grf.read_file(&format!("{base}.spr")).unwrap();
            let act = grf.read_file(&format!("{base}.act")).unwrap();
            let thumb = from_files(&spr, &act).unwrap_or_else(|| panic!("no thumbnail for {name}"));
            let mut bytes = Vec::new();
            bytes.extend(thumb.width.to_le_bytes());
            bytes.extend(thumb.height.to_le_bytes());
            bytes.extend(&thumb.rgba);
            std::fs::write(format!("{out}/{name}.rgba"), bytes).unwrap();
        }
    }

    #[test]
    fn tint_scales_the_colour() {
        let images = [solid(1, 1, [200, 200, 200, 255])];
        let mut layer = clip(0, 0, 0);
        layer.color = [255, 0, 255, 255];
        let t = compose(&images, 1, &motion(vec![layer])).unwrap();
        assert_eq!(pixel(&t, 0, 0), [200, 0, 200, 255]);
    }
}
