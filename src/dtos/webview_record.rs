use curia::{Fields, Level, LogEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The target every record from the webview is attributed to.
pub const WEBVIEW_TARGET: &str = "webview";

/// One log record as the webview sends it across the command boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebviewRecord {
    pub level: Level,
    pub message: String,
    // serde_json::Map, not HashMap<String, String>. That single choice is what
    // lets a number stay a number and a nested object stay nested.
    pub fields: Option<Map<String, Value>>,
    pub location: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl WebviewRecord {
    /// Turns the payload into the event the logger understands.
    pub fn into_event(self) -> LogEvent {
        let target = match &self.location {
            Some(location) => format!("{WEBVIEW_TARGET}:{location}"),
            None => WEBVIEW_TARGET.to_string(),
        };

        LogEvent {
            level: self.level,
            target,
            message: self.message,
            fields: self.fields.map(Fields::from_map).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
            file: self.file,
            line: self.line,
        }
    }
}
