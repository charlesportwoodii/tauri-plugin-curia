use curia::{Dispatcher, Level, LogEvent, Logger, Sink};
use std::sync::{Arc, Mutex};

struct Capture(Arc<Mutex<Vec<LogEvent>>>);

impl Sink for Capture {
    fn level(&self) -> Level {
        Level::Trace
    }

    fn emit(&self, event: &LogEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

// The macros are logger's; this asserts only that the plugin's re-export gives
// them a working second path, so a consumer needs one dependency.
#[test]
fn the_macros_are_reachable_through_the_plugin() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    Logger::install(Box::new(Dispatcher::new(vec![Capture(captured.clone())]))).expect("install");

    tauri_plugin_curia::warn!("through the plugin", { device_host: "asio" });

    let events = captured.lock().unwrap();
    let event = events
        .iter()
        .find(|e| e.message == "through the plugin")
        .expect("captured");

    assert_eq!(event.level, Level::Warn);
    assert_eq!(
        event.fields.get("device_host").unwrap(),
        &serde_json::json!("asio")
    );
}
