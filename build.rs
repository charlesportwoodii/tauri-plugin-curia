const COMMANDS: &[&str] = &["log"];

fn main() {
    // No global_api_script_path: the consumer runs with withGlobalTauri false.
    // No ios_path: the iOS console goes through the oslog crate rather than a
    // Swift shim, so there is no Swift package to build.
    tauri_plugin::Builder::new(COMMANDS).build();
}
