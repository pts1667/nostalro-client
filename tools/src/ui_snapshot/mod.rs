//! Renders UI windows offscreen with fake data and writes PNG files. Lets you
//! look at a window, or compare before and after a change, without launching
//! the client. See `docs/ui-snapshots.md`.

mod gpu;
mod scenario;

use gpu::{Headless, UiPipeline, build_font};
use ragnarok_formats::grf::GrfArchive;
use ragnarok_game::data_table::item_resource_table::ItemResourceTable;
use ragnarok_renderer::{FontAtlas, UiDrawCall, UiTextureRef};
use ragnarok_resources::font::{NANUM_BARUN_GOTHIC, NANUM_BARUN_GOTHIC_BOLD};
use ragnarok_resources::grf::DEFAULT_ARCHIVE;
use ragnarok_ui::context::UiContext;
use ragnarok_ui::frame::UiFrame;
use ragnarok_ui::state::StateCache;
use ragnarok_ui_component_hot::{CATEGORY_NAMES, SCENE_NAMES, Scene};
use scenario::{Step, parse_key, parse_point};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const USAGE: &str = "\
usage: ui-snapshot [SCENE...] [options]

Renders UI windows offscreen with fake data and writes one PNG per scene.
SCENE is a window name (see --list), or category:NAME for a gallery of windows.

options:
  --all                  every single window
  --list                 print scene and category names and exit
  --mode fallback|grf    fallback (default): no data files, procedural look.
                         grf: textured look, needs --grf
  --grf PATH             GRF archive, repeatable (grf mode, default data/data.grf)
  --data-dir PATH        extracted data folder that overrides the archives (grf mode)
  --out DIR              output root (default target/ui-snapshots)
  --size WxH             output image size in pixels (default 1280x900)
  --dpi F                UI scale (default 1.0)
  --bg RRGGBB            background colour (default 1a1a26)
  --crop                 trim to the drawn content, plus an 8px margin
  --software             use a CPU fallback adapter, for more stable output
  --hover X,Y            move the pointer to X,Y (logical pixels)
  --click X,Y            press and release at X,Y
  --type TEXT            type characters
  --key NAME             press enter, tab, escape or backspace
  --suffix NAME          append NAME to each output file name
  -h, --help             show this help

--hover, --click, --type and --key run in the order given, one or more frames each.
Output: <out>/<mode>/<scene>[<suffix>].png
";

/// Frames to run before a final snapshot, so windows that load textures
/// after the first build get their real sizes.
const MAX_SETTLE_PASSES: usize = 4;
const CROP_MARGIN: u32 = 8;
const DEFAULT_SIZE: (u32, u32) = (1280, 900);
const DEFAULT_BG: [u8; 4] = [0x1a, 0x1a, 0x26, 0xff];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fallback,
    Grf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Single(String),
    Category(String),
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub targets: Vec<Target>,
    pub mode: Mode,
    pub grf: Vec<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub out: PathBuf,
    pub size: (u32, u32),
    pub dpi: f32,
    pub background: [u8; 4],
    pub crop: bool,
    pub software: bool,
    pub steps: Vec<Step>,
    pub suffix: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            mode: Mode::Fallback,
            grf: Vec::new(),
            data_dir: None,
            out: PathBuf::from("target/ui-snapshots"),
            size: DEFAULT_SIZE,
            dpi: 1.0,
            background: DEFAULT_BG,
            crop: false,
            software: false,
            steps: Vec::new(),
            suffix: String::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    List,
    Run(Options),
}

/// Entry point for the `ui-snapshot` binary. `args` excludes the program name.
pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let opts = match parse_args(args)? {
        Command::Help => {
            print!("{USAGE}");
            return Ok(());
        }
        Command::List => {
            print_list();
            return Ok(());
        }
        Command::Run(opts) => opts,
    };

    let shared = load_shared(&opts)?;
    eprintln!("adapter: {}", shared.gpu.adapter_name);
    let dir = opts.out.join(match opts.mode {
        Mode::Fallback => "fallback",
        Mode::Grf => "grf",
    });
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    for target in &opts.targets {
        let mut session = Session::new(&shared, &opts, target)?;
        session.settle();
        for step in &opts.steps {
            session.step(step);
        }
        let calls = session.settle();
        let stem = target_stem(target);
        if opts.mode == Mode::Fallback {
            report_grf_textures(&stem, &calls);
        }
        let mut image = session.pipe.render(&shared.gpu, &calls)?;
        if content_bounds(&image, opts.background).is_none() {
            eprintln!("warning: {stem} rendered only the background");
        }
        if opts.crop {
            image = crop_to_content(&image, opts.background, CROP_MARGIN);
        }
        let path = dir.join(format!("{stem}{}.png", opts.suffix));
        image
            .save(&path)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut it = args.into_iter();
    let mut o = Options::default();
    let (mut help, mut list, mut grf_set) = (false, false, false);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "--list" => list = true,
            "--all" => {
                o.targets
                    .extend(SCENE_NAMES.iter().map(|n| Target::Single(n.to_string())));
            }
            "--crop" => o.crop = true,
            "--software" => o.software = true,
            "--mode" => {
                o.mode = match value(&mut it, &arg)?.as_str() {
                    "fallback" => Mode::Fallback,
                    "grf" => Mode::Grf,
                    other => return Err(format!("unknown mode '{other}' (fallback or grf)")),
                }
            }
            "--grf" => {
                o.grf.push(PathBuf::from(value(&mut it, &arg)?));
                grf_set = true;
            }
            "--data-dir" => o.data_dir = Some(PathBuf::from(value(&mut it, &arg)?)),
            "--out" => o.out = PathBuf::from(value(&mut it, &arg)?),
            "--size" => o.size = parse_size(&value(&mut it, &arg)?)?,
            "--dpi" => o.dpi = parse_dpi(&value(&mut it, &arg)?)?,
            "--bg" => o.background = parse_color(&value(&mut it, &arg)?)?,
            "--hover" => {
                let (x, y) = parse_point(&value(&mut it, &arg)?)?;
                o.steps.push(Step::Hover { x, y });
            }
            "--click" => {
                let (x, y) = parse_point(&value(&mut it, &arg)?)?;
                o.steps.push(Step::Click { x, y });
            }
            "--type" => o.steps.push(Step::Type(value(&mut it, &arg)?)),
            "--key" => o.steps.push(Step::Key(parse_key(&value(&mut it, &arg)?)?)),
            "--suffix" => o.suffix = value(&mut it, &arg)?,
            flag if flag.starts_with('-') => {
                return Err(format!("unknown option '{flag}' (see --help)"));
            }
            name => o.targets.push(parse_target(name)),
        }
    }

    if help {
        return Ok(Command::Help);
    }
    if list {
        return Ok(Command::List);
    }
    if o.mode == Mode::Fallback {
        if grf_set {
            return Err("--grf needs --mode grf".into());
        }
        if o.data_dir.is_some() {
            return Err("--data-dir needs --mode grf".into());
        }
    } else if o.grf.is_empty() {
        o.grf.push(PathBuf::from(DEFAULT_ARCHIVE));
    }
    if o.targets.is_empty() {
        return Err("no scenes given (see --list, or --help)".into());
    }
    Ok(Command::Run(o))
}

fn value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_target(name: &str) -> Target {
    match name.strip_prefix("category:") {
        Some(category) => Target::Category(category.to_string()),
        None => Target::Single(name.to_string()),
    }
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WxH but got '{s}'"))?;
    let w: u32 = w.parse().map_err(|_| format!("bad width in '{s}'"))?;
    let h: u32 = h.parse().map_err(|_| format!("bad height in '{s}'"))?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("size '{s}' out of range (1..=8192)"));
    }
    Ok((w, h))
}

fn parse_dpi(s: &str) -> Result<f32, String> {
    let dpi: f32 = s.parse().map_err(|_| format!("bad dpi '{s}'"))?;
    if !dpi.is_finite() || dpi <= 0.0 {
        return Err(format!("dpi must be positive, got '{s}'"));
    }
    Ok(dpi)
}

fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("expected RRGGBB but got '{s}'"));
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok([byte(0), byte(2), byte(4), 0xff])
}

fn print_list() {
    println!("scenes (use as NAME):");
    for name in SCENE_NAMES {
        println!("  {name}");
    }
    println!("categories (use as category:NAME):");
    for name in CATEGORY_NAMES {
        println!("  category:{name}");
    }
}

fn target_stem(target: &Target) -> String {
    match target {
        Target::Single(name) => name.clone(),
        Target::Category(name) => format!("category-{name}"),
    }
}

/// Opens the GPU, font and (in grf mode) archives once for all scenes in a run.
struct Shared {
    gpu: Headless,
    font: FontAtlas,
    grf: Option<GrfArchive>,
    table: Option<ItemResourceTable>,
}

fn load_shared(opts: &Options) -> Result<Shared, String> {
    match opts.mode {
        Mode::Fallback => Ok(Shared {
            gpu: Headless::new(opts.software)?,
            font: build_font(opts.dpi, None),
            grf: None,
            table: None,
        }),
        Mode::Grf => {
            for path in opts.grf.iter().chain(opts.data_dir.iter()) {
                if !path.exists() {
                    return Err(format!(
                        "data not found: {} (pass --grf/--data-dir, or use --mode fallback)",
                        path.display()
                    ));
                }
            }
            let paths: Vec<String> = opts
                .grf
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let grf = GrfArchive::open_layered(&paths, opts.data_dir.as_deref())
                .map_err(|e| format!("cannot open GRF archive: {e}"))?;
            let table = ItemResourceTable::load(&grf);
            let font_bytes = [NANUM_BARUN_GOTHIC_BOLD, NANUM_BARUN_GOTHIC]
                .iter()
                .find_map(|path| grf.read_file(path).ok());
            Ok(Shared {
                gpu: Headless::new(opts.software)?,
                font: build_font(opts.dpi, font_bytes.as_deref()),
                grf: Some(grf),
                table: Some(table),
            })
        }
    }
}

/// One scene being rendered: the window state, the input context, and the
/// texture cache. Built fresh for each scene so no state leaks between them.
struct Session<'a> {
    shared: &'a Shared,
    pipe: UiPipeline,
    scene: Scene,
    ctx: UiContext,
    state: StateCache,
    positions: HashMap<u32, [f32; 2]>,
    /// Texture names already requested, so a missing one is only tried once.
    attempted: HashSet<String>,
}

impl<'a> Session<'a> {
    fn new(shared: &'a Shared, opts: &Options, target: &Target) -> Result<Self, String> {
        let scene = match target {
            Target::Single(name) => Scene::single(name)?,
            Target::Category(name) => Scene::category(name)?,
        };
        let (width, height) = opts.size;
        let mut ctx = UiContext::new(width as f32 / opts.dpi, height as f32 / opts.dpi);
        ctx.dpi_scale = opts.dpi;
        let pipe = UiPipeline::new(
            &shared.gpu,
            &shared.font,
            width,
            height,
            opts.dpi,
            opts.background,
        );
        let mut session = Self {
            shared,
            pipe,
            scene,
            ctx,
            state: StateCache::new(),
            positions: HashMap::new(),
            attempted: HashSet::new(),
        };
        session.refresh_sizes();
        Ok(session)
    }

    /// Runs one frame with no input until textures stop appearing, and returns
    /// the draw calls of the last frame.
    fn settle(&mut self) -> Vec<UiDrawCall> {
        let mut calls = self.build_frame();
        for _ in 0..MAX_SETTLE_PASSES {
            if !self.load_missing(&calls) {
                break;
            }
            calls = self.build_frame();
        }
        self.ctx.begin_frame();
        calls
    }

    /// Runs one input step. A click also gets a release frame.
    fn step(&mut self, step: &Step) {
        step.apply(&mut self.ctx);
        self.frame();
        if step.is_click() {
            self.ctx.mouse_down = false;
            self.frame();
        }
    }

    fn frame(&mut self) {
        let calls = self.build_frame();
        self.load_missing(&calls);
        self.ctx.begin_frame();
    }

    fn build_frame(&mut self) -> Vec<UiDrawCall> {
        let mut ui = UiFrame::new(
            &mut self.ctx,
            &self.shared.font,
            &mut self.state,
            0.0,
            None,
            &self.positions,
        );
        self.scene.build(&mut ui);
        ui.finish().draw_calls
    }

    /// Loads textures the frame drew that are not cached yet. Returns `true`
    /// when something new was requested. Does nothing in fallback mode, where
    /// textures draw as white.
    fn load_missing(&mut self, calls: &[UiDrawCall]) -> bool {
        let shared = self.shared;
        let Some(grf) = shared.grf.as_ref() else {
            return false;
        };
        let mut fresh = Vec::new();
        for call in calls {
            if let UiTextureRef::Named(name) = &call.texture
                && self.pipe.textures.get(name).is_none()
                && self.attempted.insert(name.clone())
            {
                fresh.push(name.clone());
            }
        }
        if fresh.is_empty() {
            return false;
        }
        for name in &fresh {
            let loaded = self.pipe.textures.get_or_load(
                name,
                grf,
                &shared.gpu.device,
                &shared.gpu.queue,
                true,
            );
            if loaded.is_none() {
                eprintln!("warning: texture not found in archive: {name}");
            }
        }
        self.refresh_sizes();
        true
    }

    /// Tells the windows about the textures loaded so far, so layout uses real sizes.
    fn refresh_sizes(&mut self) {
        let shared = self.shared;
        if shared.grf.is_none() {
            return;
        }
        let textures = &self.pipe.textures;
        let size_fn = |name: &str| textures.texture_size(name);
        self.scene
            .enable_grf_textures(&size_fn, shared.table.as_ref());
    }
}

/// Smallest box (inclusive corners) holding every pixel that differs from
/// `background`, or `None` if the image is only background.
fn content_bounds(image: &image::RgbaImage, background: [u8; 4]) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0 != background {
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    bounds
}

/// Lists the GRF textures a frame draws. In fallback mode they appear as white
/// boxes, so this shows what still depends on the data files.
fn report_grf_textures(stem: &str, calls: &[UiDrawCall]) {
    let mut names: Vec<&str> = calls
        .iter()
        .filter_map(|call| match &call.texture {
            UiTextureRef::Named(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        return;
    }
    eprintln!(
        "note: {stem} draws {} GRF texture(s) without data (skipped in fallback):",
        names.len()
    );
    for name in names {
        eprintln!("  {name}");
    }
}

/// Trims `image` to the area that differs from `background`, plus `margin`.
fn crop_to_content(image: &image::RgbaImage, background: [u8; 4], margin: u32) -> image::RgbaImage {
    let (w, h) = image.dimensions();
    let Some((x0, y0, x1, y1)) = content_bounds(image, background) else {
        return image.clone();
    };
    let x0 = x0.saturating_sub(margin);
    let y0 = y0.saturating_sub(margin);
    let x1 = (x1 + 1 + margin).min(w);
    let y1 = (y1 + 1 + margin).min(h);
    image::imageops::crop_imm(image, x0, y0, x1 - x0, y1 - y0).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    fn options(args: &[&str]) -> Options {
        match run_args(args) {
            Ok(Command::Run(o)) => o,
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn defaults_to_fallback_at_1280x900() {
        let o = options(&["login"]);
        assert_eq!(o.mode, Mode::Fallback);
        assert_eq!(o.size, (1280, 900));
        assert_eq!(o.dpi, 1.0);
        assert_eq!(o.targets, vec![Target::Single("login".into())]);
        assert!(o.grf.is_empty());
    }

    #[test]
    fn grf_mode_defaults_to_data_grf() {
        let o = options(&["login", "--mode", "grf"]);
        assert_eq!(o.grf, vec![PathBuf::from(DEFAULT_ARCHIVE)]);
    }

    #[test]
    fn steps_keep_command_line_order() {
        let o = options(&[
            "inventory",
            "--hover",
            "4,5",
            "--click",
            "6,7",
            "--type",
            "ab",
            "--key",
            "esc",
        ]);
        assert_eq!(
            o.steps,
            vec![
                Step::Hover { x: 4.0, y: 5.0 },
                Step::Click { x: 6.0, y: 7.0 },
                Step::Type("ab".into()),
                Step::Key(scenario::Key::Escape),
            ]
        );
    }

    #[test]
    fn category_prefix_selects_gallery() {
        let o = options(&["category:game", "chat"]);
        assert_eq!(
            o.targets,
            vec![
                Target::Category("game".into()),
                Target::Single("chat".into())
            ]
        );
    }

    #[test]
    fn all_adds_every_single_scene() {
        let o = options(&["--all"]);
        assert_eq!(o.targets.len(), SCENE_NAMES.len());
        assert!(o.targets.iter().all(|t| matches!(t, Target::Single(_))));
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(run_args(&["login", "--size", "0x10"]).is_err());
        assert!(run_args(&["login", "--size", "big"]).is_err());
        assert!(run_args(&["login", "--dpi", "0"]).is_err());
        assert!(run_args(&["login", "--bg", "12345"]).is_err());
        assert!(run_args(&["login", "--click", "10"]).is_err());
        assert!(run_args(&["login", "--mode", "vulkan"]).is_err());
        assert!(run_args(&["login", "--frobnicate"]).is_err());
        assert!(run_args(&["login", "--grf", "x.grf"]).is_err());
        assert!(run_args(&["login", "--data-dir", "x"]).is_err());
        assert!(run_args(&["--out", "x"]).is_err());
        assert!(run_args(&["--out"]).is_err());
    }

    #[test]
    fn help_and_list_short_circuit() {
        assert_eq!(run_args(&["--help"]), Ok(Command::Help));
        assert_eq!(run_args(&["--list"]), Ok(Command::List));
    }

    #[test]
    fn parses_colour_and_size() {
        assert_eq!(parse_color("#102030"), Ok([0x10, 0x20, 0x30, 0xff]));
        assert_eq!(parse_size("320X240"), Ok((320, 240)));
    }

    #[test]
    fn crop_keeps_content_and_margin() {
        let mut img = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 255]));
        for y in 4..6 {
            for x in 3..5 {
                img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let cropped = crop_to_content(&img, [0, 0, 0, 255], 1);
        assert_eq!(cropped.dimensions(), (4, 4));
        assert_eq!(cropped.get_pixel(1, 1).0, [255, 0, 0, 255]);
    }

    #[test]
    fn crop_of_blank_image_is_unchanged() {
        let img = image::RgbaImage::from_pixel(6, 5, image::Rgba([9, 9, 9, 255]));
        let cropped = crop_to_content(&img, [9, 9, 9, 255], 2);
        assert_eq!(cropped.dimensions(), (6, 5));
    }

    /// Renders a real window through the GPU. Skips when no adapter exists, so
    /// machines and CI without a GPU still pass.
    #[test]
    fn renders_login_to_png() {
        if Headless::new(false).is_err() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let dir = std::env::temp_dir().join(format!("ui-snapshot-test-{}", std::process::id()));
        let args = ["login", "--size", "320x240", "--out", dir.to_str().unwrap()];
        run(args.iter().map(|s| s.to_string())).expect("snapshot run");
        let png = dir.join("fallback").join("login.png");
        let img = image::open(&png).expect("png written").to_rgba8();
        assert_eq!(img.dimensions(), (320, 240));
        let distinct = img.pixels().map(|p| p.0).collect::<HashSet<_>>();
        assert!(distinct.len() > 1, "snapshot is a single flat colour");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
