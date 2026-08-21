# Clean-room provenance record: log rotation

Project: `tauri-plugin-curia`
Date: 2026-08-20
Target license: BSD-3-Clause

## Why

Part of the log rotation implementation was copied into this repository from
`tauri-plugin-log` 2.9.0 (the Tauri Programme within The Commons Conservancy),
licensed MIT OR Apache-2.0. Both licenses require the original notices to
travel with the copied code, which prevents the crate from shipping as
BSD-3-Clause alone. The decision was to replace the copied code rather than
carry the upstream terms.

## What was found to be copied

Determined by inspection on 2026-08-20.

| Path | Verdict |
|---|---|
| `src/rotating_file.rs` | Copied expression: structure, identifiers, and comments. |
| `src/strategy/rotation.rs` | Copied expression: variant names and doc comments. |
| `src/strategy/file_open.rs` | Copied expression: variant names and doc comments. |
| `src/strategy/timezone.rs` | Copied expression: type, method, and comments. |
| `src/dtos/record_payload.rs`, `WEBVIEW_TARGET`, `log://log` | Inherited wire-format identifiers. Retained deliberately; see spec section 11. |
| `src/sink/file.rs`, `src/sink/console.rs`, `src/sink/webview.rs`, `src/commands.rs`, `src/format.rs`, `src/plugin.rs`, `src/error.rs` | Original to this project. Upstream builds on `fern`; none of this code resembles it. |

## Process used

A two-team separation.

**Tainted party (specifier).** Read the copied code in order to describe its
externally observable behavior. Wrote the functional specification at
`docs/superpowers/specs/2026-08-20-rotation-clean-room-design.md`. That
document contains behavior, interface names required for compatibility, and
input/output tables. It contains no code from the original and no description
of the original's internal structure. The specifier did not write any of the
replacement implementation.

**Clean party (implementer).** A separate agent with no exposure to the
original code or to this conversation. Given only the specification, the
consuming code (`src/sink/file.rs`), the crate's error type, and the `time`
crate's public documentation. Explicitly instructed not to seek out
`tauri-plugin-log`, not to fetch it from any registry or host, and not to
recover deleted files from git.

**Removal before implementation.** The copied files were deleted from both the
working tree and the git index *before* the implementer was given the task, so
they were not present to be read.

Note: the copied files were never committed. This repository had no commits at
the time of removal, so the material exists in no commit, branch, or tag. The
blobs remain reachable only as unreferenced git objects until a garbage
collection pass removes them.

**Review.** The specifier reviewed the result against the specification only,
checking conformance to the stated behavior. The review did not compare the
result against the original.

## Deviations from the original's behavior

The replacement is not a bug-for-bug reproduction. Two defects present in the
copied code were identified during specification and deliberately excluded:

1. `RotationStrategy::KeepSome(0)` caused a `usize` subtraction underflow.
2. Two rotations within the same second collided on a filename, patched with a
   `.bak` rename that could silently destroy a previously saved `.bak`.

Archive naming was also changed (millisecond precision, numeric collision
suffixes instead of `.bak`), as was the retention behavior for `KeepOne` and
`KeepSome(0)`, which no longer delete archives created by a previous
configuration.

## Files removed

- `src/rotating_file.rs`
- `src/strategy/rotation.rs`
- `src/strategy/file_open.rs`
- `src/strategy/timezone.rs`

## Task Completed

This record is complete as of the review sign-off noted in
`docs/findings/rotation-clean-room/review.md`.
