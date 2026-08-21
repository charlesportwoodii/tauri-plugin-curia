# @charlesportwoodii/tauri-plugin-curia

Structured logging for Tauri, over the [`curia`](https://github.com/charlesportwoodii/curia) crate.

Fields cross the webview boundary as JSON, so nested objects, arrays and numeric
types survive intact. The plugin registers one command and ships the sinks that
are genuinely Tauri-coupled. It installs no sinks itself — the consumer decides
which it wants and at what levels.

## Installing

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_curia::init())
```

That registers the command. Logging does nothing until a dispatcher is installed:

```rust
use std::sync::Arc;

use curia::{Dispatcher, Level, LogEvent, Logger, Sink, TracingBridge};
use tauri::{Manager, Wry};
use tauri_plugin_curia::{ConsoleSink, FileSink, LineFormatter, WebviewSink};
use tracing_subscriber::prelude::*;

pub enum AppSink {
    Console(ConsoleSink),
    File(FileSink),
    Webview(WebviewSink<Wry>),
}

impl Sink for AppSink {
    fn level(&self) -> Level {
        match self {
            Self::Console(s) => s.level(),
            Self::File(s) => s.level(),
            Self::Webview(s) => s.level(),
        }
    }

    fn emit(&self, event: &LogEvent) {
        match self {
            Self::Console(s) => s.emit(event),
            Self::File(s) => s.emit(event),
            Self::Webview(s) => s.emit(event),
        }
    }
}

let human: LineFormatter = Arc::new(|e: &LogEvent| {
    format!("[{}][{}] {}", e.target, e.level.as_str(), e.message)
});

Logger::install(Box::new(Dispatcher::new(vec![
    AppSink::Console(ConsoleSink::new(Level::Info, human.clone())),
    AppSink::File(FileSink::new(
        app.path().app_log_dir()?,
        "app".to_string(),
        Level::Debug,
        human.clone(),
    )?),
])))?;

tracing_subscriber::registry().with(TracingBridge::to_global()).init();
TracingBridge::install_log_capture()?;
```

An enum rather than `Vec<Box<dyn Sink>>` keeps dynamic dispatch to the single
`Box<dyn Dispatch>` at the global boundary.

## Logging from Rust

```rust
use tauri_plugin_curia::{info, warn};

info!("stream started");

warn!("capture stream ended", { device_host: "asio", frames: 0 });
```

The macros are defined once, in `curia`, and re-exported here so one dependency
is enough. `tauri_plugin_curia::logger` is also re-exported for anything the plugin
does not surface directly.

There is no format-argument form. `warn!("failed {}", e)` is unsupported on
purpose: an interpolated message becomes a distinct issue per interpolation in
any error tracker downstream. Put the varying part in a field, or use
`warn!(format!("failed: {e:?}"), {})` as an escape hatch.

## Logging from TypeScript

```typescript
import { info, warn, error } from "@charlesportwoodii/tauri-plugin-curia";

await info("stream started");

await warn("capture stream ended", {
  device_host: "asio",
  frames: 0,
  live: true,
  detail: { ok: true, codes: [1, 2, 3] },
});
```

Fields are the second positional argument. `LogFields` is recursive, so nested
objects and arrays are permitted and arrive on the Rust side with their types
intact — a number stays a number.

Records logged from the webview get the target `webview:<location>`, where
`location` is recovered from the call stack. In a minified bundle the location,
file and line are simply absent.

## Sinks

| Sink | Writes to | Notes |
| --- | --- | --- |
| `ConsoleSink` | stderr, `android_logger`, `oslog` | Chosen by target platform |
| `FileSink` | A rotating file | Takes a directory; resolves nothing itself |
| `WebviewSink` | The `log://log` event | Inert unless something listens |

Every sink takes a `LineFormatter` — an `Arc<dyn Fn(&LogEvent) -> String>` — so
the same sink serves JSON and human output. The sink renders a line; it does not
decide what the line says.

**No sink blocks its caller.** `FileSink` hands lines to a worker thread through
a bounded queue; `ConsoleSink` writes to an already-open handle; `WebviewSink`
spawns. A logging call is safe on an audio or network hot path.

### FileSink

`FileSink` takes a `PathBuf` rather than resolving `app_log_dir()` itself, which
keeps it testable without a Tauri runtime. Pass `app.path().app_log_dir()?`.

Rotation is the upstream `tauri-plugin-log` implementation, unchanged:
`RotationStrategy`, `TimezoneStrategy` and `FileOpenStrategy` behave exactly as
they did, and `FileSink::with_rotation` exposes all three.

Formatting happens on the calling thread and the queue carries the finished
line, so a burst costs one allocation and a channel send. The worker writes and
flushes each record, matching what fern's writer output did upstream. That flush
is a single `write` syscall into the OS page cache, not an `fsync` — `File::flush`
is a no-op in std. The point is that the file stays complete when the process
dies without unwinding, which is the case log files are read for.

If the queue fills, lines are dropped and counted rather than blocking a
producer. `FileSink::dropped()` reports the count, and `FileSink::with_capacity`
sets the bound. Dropping the sink drains the queue and joins the worker, so the
tail reaches disk.

### WebviewSink

Emits a `log://log` event carrying `RecordPayload { message, level }`. With no
listener it spawns a task per record for nothing, so register it only alongside
one.

## Development

| Task | Runs |
| --- | --- |
| `mise run fmt` | `cargo fmt --all` |
| `mise run fmt-check` | Fails on unformatted code |
| `mise run lint` | `cargo clippy --all-targets -- -D warnings` |
| `mise run test` | `cargo test` |
| `mise run js-build` | Builds `dist-js` from `guest-js` |
| `mise run check` | All of the above, in the order CI runs them |

## A note on dist-js

`dist-js` is a build artifact, and a consumer linking this package with `link:`
gets whatever is on disk. Renaming a command in `guest-js` and rebuilding only
the Rust side ships a bundle that invokes the old path, which surfaces as
`<old-name>.log not allowed` at runtime rather than as a build error.

The `prepare` script rebuilds it on `yarn install` for exactly this reason. Run
`mise run check` rather than `cargo build` alone when changing anything the
webview calls.

## Credit

This plugin follows the design of
[`tauri-plugin-log`](https://github.com/tauri-apps/plugins-workspace) by the
Tauri Programme within The Commons Conservancy, and keeps its configuration
names so that moving between the two does not mean rewriting a config file.

No code is shared. The rotation implementation here was written from a
functional specification by an implementer with no access to the original; the
specification, the review, and the provenance record are in
[`docs/findings/rotation-clean-room/`](docs/findings/rotation-clean-room/).

Upstream reached `os_log` on iOS through a Swift package. This crate uses the
`oslog` crate instead, so both mobile platforms are pure Rust and there is no
SwiftPM target to build.

## License

BSD-3-Clause. See [`LICENSE`](LICENSE).

No third-party code is vendored into this crate.
