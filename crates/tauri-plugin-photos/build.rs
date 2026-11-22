// The Swift side (ios/) is compiled and linked into iOS builds by tauri-plugin.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).ios_path("ios").build();
}
