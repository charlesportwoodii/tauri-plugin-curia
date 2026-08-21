use curia::{Level, LogEvent, Sink};
use tauri::{AppHandle, Emitter, Runtime};

use crate::{LineFormatter, RecordPayload};

pub struct WebviewSink<R: Runtime> {
    app: AppHandle<R>,
    level: Level,
    format: LineFormatter,
}

impl<R: Runtime> WebviewSink<R> {
    pub fn new(app: AppHandle<R>, level: Level, format: LineFormatter) -> Self {
        Self { app, level, format }
    }
}

impl<R: Runtime> Sink for WebviewSink<R> {
    fn level(&self) -> Level {
        self.level
    }

    fn emit(&self, event: &LogEvent) {
        let payload = RecordPayload {
            message: (self.format)(event),
            level: event.level,
        };

        // emit must not block, so this spawns rather than awaiting
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = app.emit("log://log", payload);
        });
    }
}
