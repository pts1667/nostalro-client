// rust-i18n embeds `locales/` at compile time but does not tell cargo which files it
// read, so an edited translation would otherwise be ignored until a clean build.
fn main() {
    println!("cargo:rerun-if-changed=locales");
}
