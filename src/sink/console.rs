use curia::{Level, LogEvent, Sink};

use crate::LineFormatter;

pub struct ConsoleSink {
    level: Level,
    format: LineFormatter,
    // os_log is the only destination visible in Console.app and Xcode; stderr
    // goes nowhere on iOS. OsLog is Send + Sync and the default log's Drop is a
    // no-op, so one instance is held for the sink's lifetime.
    #[cfg(target_os = "ios")]
    os_log: oslog::OsLog,
}

impl ConsoleSink {
    pub fn new(level: Level, format: LineFormatter) -> Self {
        Self {
            level,
            format,
            #[cfg(target_os = "ios")]
            os_log: oslog::OsLog::global(),
        }
    }
}

impl Sink for ConsoleSink {
    fn level(&self) -> Level {
        self.level
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn emit(&self, event: &LogEvent) {
        use std::io::Write;

        let line = (self.format)(event);
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{line}");
    }

    // android_logger consumes a log::Record, so one is built here rather than
    // the crate carrying a log dependency on every platform.
    #[cfg(target_os = "android")]
    fn emit(&self, event: &LogEvent) {
        let line = (self.format)(event);
        let level = match event.level {
            Level::Error => log::Level::Error,
            Level::Warn => log::Level::Warn,
            Level::Info => log::Level::Info,
            Level::Debug => log::Level::Debug,
            Level::Trace => log::Level::Trace,
        };

        android_logger::log(
            &log::Record::builder()
                .level(level)
                .target(&event.target)
                .args(format_args!("{line}"))
                .build(),
        );
    }

    #[cfg(target_os = "ios")]
    fn emit(&self, event: &LogEvent) {
        let line = (self.format)(event);

        match event.level {
            Level::Error => self.os_log.error(&line),
            Level::Warn => self.os_log.default(&line),
            Level::Info => self.os_log.info(&line),
            Level::Debug | Level::Trace => self.os_log.debug(&line),
        }
    }
}
