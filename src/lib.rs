//! Structured logging for Tauri, over the `logger` crate.
//!
//! The plugin registers a command that turns webview calls into `curia::LogEvent`
//! and ships the sinks that are genuinely Tauri-coupled. It installs no sinks
//! itself; the consumer calls `curia::Logger::install`.

mod commands;
mod dtos;
mod error;
mod format;
mod plugin;
mod rotation;
mod sink;
mod strategy;

// Re-exported so a consumer needs one dependency rather than two. Upstream did
// the same with `pub use log` and `pub use fern`.
pub use curia;

// The macros are defined once, in `logger`. These re-exports only give them a
// second path, so a consumer writes `tauri_plugin_log::warn!` without taking a
// direct dependency; `tauri_plugin_log::logger` remains available for anything
// the plugin does not surface.
pub use curia::{debug, error, info, trace, warn};

pub use dtos::{RecordPayload, WEBVIEW_TARGET, WebviewRecord};
pub use error::Error;
pub use format::LineFormatter;
pub use rotation::RotatingFile;
pub use sink::{ConsoleSink, FileSink, WebviewSink};
pub use strategy::{FileOpenStrategy, RotationStrategy, TimezoneStrategy};

pub const DEFAULT_MAX_FILE_SIZE: u64 = 40_000;
pub const DEFAULT_ROTATION_STRATEGY: RotationStrategy = RotationStrategy::KeepOne;
pub const DEFAULT_TIMEZONE_STRATEGY: TimezoneStrategy = TimezoneStrategy::UseUtc;
pub const DEFAULT_FILE_OPEN_STRATEGY: FileOpenStrategy = FileOpenStrategy::Append;

// A free function by framework convention: every Tauri plugin is mounted as
// `tauri_plugin_<name>::init()` in the consumer's builder chain. It holds no
// logic of its own, delegating to `Plugin::init`.
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    plugin::Plugin::init()
}
