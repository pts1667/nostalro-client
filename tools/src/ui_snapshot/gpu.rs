//! Headless wgpu setup for UI snapshots: a device with no surface, one offscreen
//! `Rgba8Unorm` target, and a readback to CPU pixels.
//!
//! `Rgba8Unorm` (not sRGB) is required: `ui.wgsl` converts linear to sRGB itself,
//! so an sRGB target would apply the conversion twice.

use image::{Rgba, RgbaImage};
use ragnarok_renderer::font_atlas::euc_kr_charset;
use ragnarok_renderer::texture::{self, TextureCache};
use ragnarok_renderer::{FontAtlas, UiDrawCall, UiRenderer, UiTextureRef, block_on, wgpu};
use std::sync::mpsc;

pub const SNAPSHOT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Headless {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
}

impl Headless {
    /// `software` asks for a CPU fallback adapter (lavapipe, WARP) so output is
    /// more stable across machines. Honours `WGPU_BACKEND` and friends.
    pub fn new(software: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all().with_env(),
            ..Default::default()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: software,
        }))
        .map_err(|e| format!("no GPU adapter ({e}); try --software"))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ui-snapshot"),
            ..Default::default()
        }))
        .map_err(|e| format!("request_device failed: {e}"))?;
        Ok(Self {
            device,
            queue,
            adapter_name,
        })
    }
}

/// Font atlas for UI text. Uses the embedded CJK font, which is what the client
/// falls back to without GRF, so both modes lay out text the same way.
pub fn build_font(dpi: f32, grf_font: Option<&[u8]>) -> FontAtlas {
    let extra_chars = euc_kr_charset();
    match grf_font {
        Some(data) => FontAtlas::build_with_extra_chars(data, FONT_PX_HEIGHT, dpi, &extra_chars),
        None => FontAtlas::from_embedded_cjk(FONT_PX_HEIGHT, dpi, &extra_chars),
    }
}

/// Matches the UI font size the windowed client uses.
pub const FONT_PX_HEIGHT: f32 = 14.0;

/// Owns the UI renderer and texture cache for one snapshot target size.
pub struct UiPipeline {
    renderer: UiRenderer,
    pub textures: TextureCache,
    font_bind: wgpu::BindGroup,
    white_bind: wgpu::BindGroup,
    width: u32,
    height: u32,
    background: [u8; 4],
}

impl UiPipeline {
    /// `width`/`height` are the output image size in pixels. The UI itself lays
    /// out in logical pixels, so the logical size is `width / dpi` by `height / dpi`.
    pub fn new(
        gpu: &Headless,
        font: &FontAtlas,
        width: u32,
        height: u32,
        dpi: f32,
        background: [u8; 4],
    ) -> Self {
        let textures = TextureCache::new(&gpu.device);
        let layout = &textures.bind_group_layout;
        let renderer = UiRenderer::new(
            &gpu.device,
            SNAPSHOT_FORMAT,
            layout,
            width as f32 / dpi,
            height as f32 / dpi,
        );
        let font_bind = texture::create_font_atlas_bind_group(
            &gpu.device,
            &gpu.queue,
            &font.image,
            layout,
            "ui-snapshot-font",
        );
        let white = RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let white_bind = texture::create_texture_bind_group(
            &gpu.device,
            &gpu.queue,
            &white,
            layout,
            "ui-snapshot-white",
        );
        Self {
            renderer,
            textures,
            font_bind,
            white_bind,
            width,
            height,
            background,
        }
    }

    /// Renders `draw_calls` onto a cleared target and reads it back. Textures
    /// that are missing from the cache draw as white, as in the client.
    pub fn render(
        &mut self,
        gpu: &Headless,
        draw_calls: &[UiDrawCall],
    ) -> Result<RgbaImage, String> {
        let device = &gpu.device;
        let (w, h) = (self.width, self.height);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui-snapshot-target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SNAPSHOT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());

        let [r, g, b, a] = self.background;
        let clear = wgpu::Color {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: a as f64 / 255.0,
        };
        let font_bind = &self.font_bind;
        let white_bind = &self.white_bind;
        let textures = &self.textures;
        let commands: Vec<ragnarok_renderer::UiDrawCommand> = draw_calls
            .iter()
            .map(|call| ragnarok_renderer::UiDrawCommand {
                vertices: &call.vertices,
                indices: &call.indices,
                texture: bind_for(&call.texture, font_bind, white_bind, textures),
            })
            .collect();

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            // Clear pass: `UiRenderer::render` loads the target rather than clearing it.
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui-snapshot-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }
        self.renderer
            .render(&mut encoder, &view, device, &gpu.queue, &commands);

        let row_bytes = w * 4;
        let padded_row = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-snapshot-readback"),
            size: (padded_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .map_err(|e| format!("readback channel closed: {e}"))?
            .map_err(|e| format!("readback map failed: {e}"))?;

        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((row_bytes * h) as usize);
        for row in 0..h as usize {
            let start = row * padded_row as usize;
            pixels.extend_from_slice(&mapped[start..start + row_bytes as usize]);
        }
        drop(mapped);
        readback.unmap();

        RgbaImage::from_raw(w, h, pixels).ok_or_else(|| "readback size mismatch".to_string())
    }
}

/// Resolves a draw call's texture to a bind group. Free function so the caller
/// can borrow the pipeline's fields separately from the renderer.
fn bind_for<'a>(
    texture: &UiTextureRef,
    font_bind: &'a wgpu::BindGroup,
    white_bind: &'a wgpu::BindGroup,
    textures: &'a TextureCache,
) -> &'a wgpu::BindGroup {
    match texture {
        UiTextureRef::FontAtlas => font_bind,
        UiTextureRef::White | UiTextureRef::Inline(_) => white_bind,
        UiTextureRef::Named(name) => textures.get(name).unwrap_or(white_bind),
    }
}
