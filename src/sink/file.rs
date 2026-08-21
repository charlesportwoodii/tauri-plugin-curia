use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

use curia::{Level, LogEvent, Sink};

use crate::{
    DEFAULT_FILE_OPEN_STRATEGY, DEFAULT_MAX_FILE_SIZE, DEFAULT_ROTATION_STRATEGY,
    DEFAULT_TIMEZONE_STRATEGY, Error, FileOpenStrategy, LineFormatter, RotatingFile,
    RotationStrategy, TimezoneStrategy,
};

// Log lines are small and the worker drains continuously, so this is burst
// headroom rather than throughput capacity.
const QUEUE_CAPACITY: usize = 4096;

pub struct FileSink {
    level: Level,
    format: LineFormatter,
    // Taken in Drop so the channel disconnects and the worker drains
    tx: Option<flume::Sender<String>>,
    dropped: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}

impl FileSink {
    pub fn new(
        dir: PathBuf,
        file_name: String,
        level: Level,
        format: LineFormatter,
    ) -> Result<Self, Error> {
        Self::with_rotation(
            dir,
            file_name,
            level,
            format,
            DEFAULT_MAX_FILE_SIZE,
            DEFAULT_ROTATION_STRATEGY,
            DEFAULT_TIMEZONE_STRATEGY,
            DEFAULT_FILE_OPEN_STRATEGY,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_rotation(
        dir: PathBuf,
        file_name: String,
        level: Level,
        format: LineFormatter,
        max_size: u64,
        rotation: RotationStrategy,
        timezone: TimezoneStrategy,
        open: FileOpenStrategy,
    ) -> Result<Self, Error> {
        Self::with_capacity(
            dir,
            file_name,
            level,
            format,
            max_size,
            rotation,
            timezone,
            open,
            QUEUE_CAPACITY,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_capacity(
        dir: PathBuf,
        file_name: String,
        level: Level,
        format: LineFormatter,
        max_size: u64,
        rotation: RotationStrategy,
        timezone: TimezoneStrategy,
        open: FileOpenStrategy,
        capacity: usize,
    ) -> Result<Self, Error> {
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        // Opened here rather than on the worker so an unwritable directory is
        // an error the caller can act on, not a thread that dies silently.
        let mut file = RotatingFile::new(&dir, file_name, max_size, rotation, timezone, open)?;

        let (tx, rx) = flume::bounded::<String>(capacity);

        let worker = std::thread::Builder::new()
            .name("log-file-sink".to_string())
            .spawn(move || {
                // recv fails once every sender is dropped, and only after the
                // queue is empty, so this drains before exiting.
                while let Ok(line) = rx.recv() {
                    let _ = writeln!(file, "{line}");

                    // RotatingFile buffers into a Vec and only hands bytes to
                    // the OS on flush. File::flush is a no-op in std, so this
                    // is one write syscall into the page cache, not an fsync —
                    // which is what keeps the file complete when the process
                    // dies without unwinding.
                    let _ = file.flush();
                }
            })
            .map_err(Error::Io)?;

        Ok(Self {
            level,
            format,
            tx: Some(tx),
            dropped: Arc::new(AtomicU64::new(0)),
            worker: Some(worker),
        })
    }

    // Lines lost to a full queue. Never silently discarded without a count.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn queued(&self) -> usize {
        self.tx.as_ref().map(|tx| tx.len()).unwrap_or(0)
    }
}

impl Sink for FileSink {
    fn level(&self) -> Level {
        self.level
    }

    fn emit(&self, event: &LogEvent) {
        // Formatting happens here rather than on the worker: the queue then
        // carries one String instead of a cloned LogEvent with its field map.
        let line = (self.format)(event);

        let Some(tx) = self.tx.as_ref() else {
            return;
        };

        // try_send, never send. A blocked producer may be an audio thread.
        if tx.try_send(line).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        // Dropping the sender disconnects the channel, which ends the worker
        // loop once it has drained. Joining then guarantees the tail is on disk.
        self.tx.take();

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
