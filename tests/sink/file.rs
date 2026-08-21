use std::sync::Arc;

use curia::{Fields, Level, LogEvent, Sink};
use tauri_plugin_curia::{
    DEFAULT_FILE_OPEN_STRATEGY, DEFAULT_ROTATION_STRATEGY, DEFAULT_TIMEZONE_STRATEGY, FileSink,
};

fn event(level: Level, message: &str) -> LogEvent {
    LogEvent {
        level,
        target: "test".to_string(),
        message: message.to_string(),
        fields: Fields::new(),
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("logsink-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_formatter_output_is_what_lands_on_disk() {
    let dir = scratch("fmt");

    let sink = FileSink::new(
        dir.clone(),
        "test".to_string(),
        Level::Trace,
        Arc::new(|e: &LogEvent| format!("<<{}>>", e.message)),
    )
    .unwrap();

    sink.emit(&event(Level::Info, "first"));
    sink.emit(&event(Level::Info, "second"));

    // Drop joins the worker, so the tail is on disk by the time this returns
    drop(sink);

    let written = std::fs::read_to_string(dir.join("test.log")).unwrap();
    assert_eq!(
        written.lines().collect::<Vec<_>>(),
        vec!["<<first>>", "<<second>>"]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_burst_past_the_queue_bound_never_blocks_and_never_loses_silently() {
    let dir = scratch("burst");
    const CAPACITY: usize = 8;
    const BURST: usize = 4096;

    let sink = FileSink::with_capacity(
        dir.clone(),
        "burst".to_string(),
        Level::Trace,
        Arc::new(|e: &LogEvent| e.message.clone()),
        // Large enough that rotation never fires and the accounting stays exact
        u64::MAX,
        DEFAULT_ROTATION_STRATEGY,
        DEFAULT_TIMEZONE_STRATEGY,
        DEFAULT_FILE_OPEN_STRATEGY,
        CAPACITY,
    )
    .unwrap();

    for i in 0..BURST {
        sink.emit(&event(Level::Info, &format!("line {i}")));
    }

    let dropped = sink.dropped() as usize;
    drop(sink);

    let written = std::fs::read_to_string(dir.join("burst.log")).unwrap();
    let landed = written.lines().count();

    // Every line is either on disk or counted. Nothing vanishes unaccounted for.
    assert_eq!(
        landed + dropped,
        BURST,
        "landed {landed} + dropped {dropped}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_sink_reports_its_registered_level() {
    let dir = scratch("lvl");

    let sink = FileSink::new(
        dir.clone(),
        "lvl".to_string(),
        Level::Warn,
        Arc::new(|e: &LogEvent| e.message.clone()),
    )
    .unwrap();

    assert_eq!(sink.level(), Level::Warn);

    drop(sink);
    std::fs::remove_dir_all(&dir).ok();
}
