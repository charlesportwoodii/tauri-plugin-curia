//! Structured logging for Tauri, over the `logger` crate.
//!
//! The plugin registers a command that turns webview calls into `curia::LogEvent`
//! and ships the one sink that is genuinely Tauri-coupled. It installs no sinks
//! itself; the consumer calls `curia::Logger::install`.

mod commands;
mod dtos;
mod error;
mod plugin;
mod sink;

// Re-exported so a consumer needs one dependency rather than two. Upstream did
// the same with `pub use log` and `pub use fern`.
pub use curia;

// The macros are defined once, in `logger`. These re-exports only give them a
// second path, so a consumer writes `tauri_plugin_log::warn!` without taking a
// direct dependency; `tauri_plugin_log::logger` remains available for anything
// the plugin does not surface.
pub use curia::{debug, error, info, trace, warn};

// The sinks and the rotation machinery live in curia, which has no Tauri
// dependency. They are re-exported so a consumer of this plugin still reaches
// them by one path.
pub use curia::{
    ConsoleSink, DEFAULT_FILE_OPEN_STRATEGY, DEFAULT_MAX_FILE_SIZE, DEFAULT_ROTATION_STRATEGY,
    DEFAULT_TIMEZONE_STRATEGY, FileOpenStrategy, FileSink, LineFormatter, RotatingFile,
    RotationStrategy, TimezoneStrategy,
};

pub use dtos::{RecordPayload, WEBVIEW_TARGET, WebviewRecord};
pub use error::Error;
pub use sink::WebviewSink;

// A free function by framework convention: every Tauri plugin is mounted as
// `tauri_plugin_<name>::init()` in the consumer's builder chain. It holds no
// logic of its own, delegating to `Plugin::init`.
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    plugin::Plugin::init()
}
