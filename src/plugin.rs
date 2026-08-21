use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

use crate::commands;

pub struct Plugin;

impl Plugin {
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        // Must match the permission identifier tauri-plugin derives from the crate's
        // links value ("tauri-plugin-bvc-log" -> "bvc-log"), or the capability
        // grant never applies to the command.
        Builder::new("curia")
            .invoke_handler(tauri::generate_handler![commands::log])
            .build()
    }
}
