# UI snapshots

`ui-snapshot` renders UI windows offscreen, with fake data, and writes each one
to a PNG. No window opens. Use it to see a window's current state, and to
compare before and after a UI change.

It builds the same windows as the hot-reload viewer
(`tools/ui-component-dev.sh`), from the fixtures in `tools/ui-component-hot`.
It needs a GPU adapter, which any normal desktop has. It does not run in CI.

## Quick start

```bash
# list every scene and category
cargo run --bin ui-snapshot -- --list

# one window, procedural look, no data files needed
cargo run --bin ui-snapshot -- inventory

# several windows and a category gallery
cargo run --bin ui-snapshot -- login chat category:game

# every single window, into a folder you can compare later
cargo run --bin ui-snapshot -- --all --out target/ui-before
```

Output goes to `target/ui-snapshots/<mode>/<scene>.png` by default. A category
gallery is written as `category-<name>.png`.

## Render modes

| Mode | Flag | What it draws |
| --- | --- | --- |
| Fallback (default) | `--mode fallback` | The procedural look from `lib/ui-core/src/theme.rs` and `lib/ui-component/src/helper/fallback.rs`. Never opens a data file. |
| Game art | `--mode grf` | The same procedural chrome, plus game art (item, skill and emote icons, map images) from the data files. Needs `--grf`, or uses `data/data.grf` when that is omitted. |

Fallback mode is the one that matters for the GRF-free UI. The chrome never reads
the data files, in either mode. Game art does: in fallback mode a game texture
that is missing is not drawn, and the renderer logs one error per texture name.
Every run lists those textures on stderr:

```
note: emotion draws 64 game texture(s) without data (skipped in fallback):
  @emo/3
  ...
```

That list is game art only. Run `--all` to get the full picture.

GRF mode, against your kRO download:

```bash
cargo run --bin ui-snapshot -- inventory login category:game --mode grf \
  --grf /c/Users/Thoma/Downloads/kRO_FullRE_2012-02-09/data.grf
```

Add `--grf .../rdata.grf` as well if a texture is reported missing. Repeat `--grf`
to layer archives, with the first one taking priority. `--data-dir` points at an
extracted folder that overrides the archives. Both paths accept forward slashes
on Windows.

## Scenes

`SCENE` is a window name, as printed by `--list`. `category:NAME` renders a
gallery of related windows, packed the same way the hot-reload viewer lays them
out. `--all` renders every single window, not the categories.

Some windows start closed. `confirm_dialog`, for example, renders only the
background until it is opened. A warning says so.

Output from category runs includes the `[gallery]` layout lines that the
hot-reload viewer also prints. They are informational.

## Options

| Option | Default | Meaning |
| --- | --- | --- |
| `--list` | | Print scene and category names, then exit. |
| `--all` | | Add every single window. |
| `--mode fallback\|grf` | `fallback` | See [Render modes](#render-modes). |
| `--grf PATH` | `data/data.grf` in grf mode | GRF archive. Repeatable. grf mode only. |
| `--data-dir PATH` | | Extracted data folder that overrides the archives. grf mode only. |
| `--out DIR` | `target/ui-snapshots` | Output root. Files go to `DIR/<mode>/`. |
| `--size WxH` | `1280x900` | Output image size in pixels. |
| `--dpi F` | `1.0` | UI scale. The logical layout size is `size / dpi`. |
| `--bg RRGGBB` | `1a1a26` | Background colour. |
| `--crop` | | Trim to the drawn content, plus an 8px margin. |
| `--software` | | Use a CPU fallback adapter. More stable output across machines. |
| `--hover X,Y` | | Move the pointer to `X,Y`, in logical pixels. |
| `--click X,Y` | | Press and release at `X,Y`. |
| `--type TEXT` | | Type characters into the focused field. |
| `--key NAME` | | Press `enter`, `tab`, `escape` or `backspace`. |
| `--suffix NAME` | | Append `NAME` to every output file name. |

Input options run in the order given, after the window has settled. Each one is
one or more UI frames, so hover, click and typing behave as they do in the
client. A click gets a press frame and a release frame.

```bash
# the Equipment tab after a click, next to the plain snapshot
cargo run --bin ui-snapshot -- inventory --click 10,154 --suffix _eqp

# hover state
cargo run --bin ui-snapshot -- inventory --hover 60,135 --suffix _hover

# type into the username field of the login form
cargo run --bin ui-snapshot -- login --click 654,428 --type admin --suffix _typed
```

Coordinates are logical pixels, so at `--dpi 1.5` they are the same numbers as
at `--dpi 1`.

## Before and after a change

Snapshot the untouched code, make the change, and snapshot again:

```bash
cargo run --bin ui-snapshot -- --all --out target/ui-before
# ... edit the UI ...
cargo run --bin ui-snapshot -- --all --out target/ui-after
```

List the files that changed. Git Bash:

```bash
diff <(cd target/ui-before/fallback && sha256sum *.png) \
     <(cd target/ui-after/fallback && sha256sum *.png)
```

PowerShell:

```powershell
Compare-Object (Get-FileHash target/ui-before/fallback/*.png) `
               (Get-FileHash target/ui-after/fallback/*.png) -Property Hash, Path
```

Open two changed images side by side to see the difference. Any image diff
tool works.

## Determinism

Renders are byte-identical when the inputs match. Time is fixed at zero, the
background is fixed, and there is no animation. Two things can still change the
output:

- **The GPU.** Different adapters rasterise slightly differently. Compare
  snapshots taken on the same machine. `--software` uses the CPU adapter, which
  is the same everywhere it is installed.
- **The window code.** That is the point of the tool.

## How it works

`tools/src/ui_snapshot/`:

- `gpu.rs` creates a wgpu device with no surface. It renders into an offscreen
  `Rgba8Unorm` texture and reads the pixels back. The format is not sRGB because
  `ui.wgsl` does the sRGB conversion itself.
- `scenario.rs` turns `--hover`, `--click`, `--type` and `--key` into
  `UiContext` input.
- `mod.rs` parses arguments and drives a session per scene. It builds the
  window with `ragnarok_ui_component_hot::Scene`, then loads any GRF texture the
  frame draws, and re-runs the frame so layout uses the real texture sizes.

The window fixtures live in `tools/ui-component-hot/src/lib.rs`. That crate is
built as both a `cdylib` for hot reload and an `rlib` for this tool. `Scene` is
the safe entry point. `SCENE_NAMES` must match the arms of `create_single`. The
`scene_names_construct` test fails if an entry goes stale.

Adding a window to the hot-reload viewer also makes it available here: add it
to `create_single` and to `SCENE_NAMES`.

## Limits

- Needs a GPU adapter. Without one the tool exits with an error, and the
  rendering test skips itself.
- Building `ragnarok-tools` now also compiles `ragnarok-ui-component`, because the
  fixtures depend on it. Expect slower first builds.
- The fake data is hand-written in the fixtures. It is not the real character
  state. It is enough to see layout and chrome, not every state a window can be
  in.
