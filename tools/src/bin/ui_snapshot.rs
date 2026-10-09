//! Renders UI windows to PNG files without opening a window. See
//! `docs/ui-snapshots.md` for usage.

fn main() {
    if let Err(e) = ragnarok_tools::ui_snapshot::run(std::env::args().skip(1)) {
        eprintln!("ui-snapshot: {e}");
        std::process::exit(1);
    }
}
