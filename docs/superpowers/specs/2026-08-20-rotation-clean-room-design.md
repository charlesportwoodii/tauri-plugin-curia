# Rotation clean-room design

Date: 2026-08-20
Status: approved, ready to implement

## 1. Why this document exists

`tauri-plugin-curia` is licensed BSD-3-Clause. Part of its log rotation code was
copied from another project under different terms. This document is a
**functional specification**: it states what the rotation code must *do*, so
that an implementer who has never seen the original can write it.

### Rules for the implementer

You are the clean-room implementer. You must **not**:

- Read, fetch, or search for `tauri-plugin-log`, the `plugins-workspace`
  repository, or any `fern`-based log rotation code.
- Fetch anything from crates.io, docs.rs, or GitHub relating to those.
- Use `git fsck`, `git cat-file`, `git show`, or any other means to recover
  files deleted from this repository.
- Read anything under `third-party-licenses/`.

You may read: this document, `src/sink/file.rs`, `src/error.rs`, `src/lib.rs`,
`Cargo.toml`, and the `time` crate's own documentation.

If this specification is unclear or contradicts itself, **ask**. Do not go
looking for a reference implementation to resolve the ambiguity. An ambiguity
resolved by reading the original defeats the entire purpose of this document.

## 2. Required public API

These names are fixed. They are interoperability surface: `RotationStrategy`
is `Serialize`/`Deserialize`, so its variant names appear in consumer config
files, and `src/sink/file.rs` already calls this API. Keep the names, write
your own documentation for them.

```rust
pub enum RotationStrategy { KeepAll, KeepOne, KeepSome(usize) }
// derives: Debug, Clone, Serialize, Deserialize

pub enum FileOpenStrategy { Append, Rotate }
// derives: Debug, Clone, PartialEq

pub enum TimezoneStrategy { UseUtc, UseLocal }
// derives: Debug, Clone

impl TimezoneStrategy {
    pub fn get_now(&self) -> time::OffsetDateTime;
}

pub struct RotatingFile { /* private */ }

impl RotatingFile {
    pub fn new(
        dir: impl AsRef<std::path::Path>,
        file_name: String,
        max_size: u64,
        rotation_strategy: RotationStrategy,
        timezone_strategy: TimezoneStrategy,
        file_open_strategy: FileOpenStrategy,
    ) -> Result<Self, crate::Error>;
}

impl std::io::Write for RotatingFile { /* write, flush */ }
```

`get_now` returns the current time: `UseUtc` in UTC, `UseLocal` in the local
offset, falling back to UTC when the OS offset cannot be determined (this
fallback is required, not optional -- determining local offset fails on some
platforms and must never panic).

`lib.rs` must keep exporting `RotatingFile`, `RotationStrategy`,
`FileOpenStrategy`, and `TimezoneStrategy` at the same paths as today, and
`src/sink/file.rs` must compile with no changes.

## 3. Module layout

```
src/rotation/mod.rs      re-exports
src/rotation/policy.rs   pure decisions: no std::fs, no clock, no I/O
src/rotation/naming.rs   archive names: build and parse. no std::fs
src/rotation/writer.rs   RotatingFile. the only module that touches disk
src/strategy/            the three enums (rewrite in place)
```

`policy.rs` and `naming.rs` must not reference `std::fs` or read the clock.
Every input they need is passed in. This is what makes them testable without
temp directories, and it is not negotiable.

## 4. The active file

The active log is `<dir>/<file_name>.log`. It is the only file written to.

- `write()` appends to an in-memory buffer and returns `Ok(buf.len())`. Nothing
  reaches the operating system.
- `flush()` on an empty buffer is a successful no-op.
- `flush()` applies the write decision (section 6), then writes the whole
  buffer to the active file and clears the buffer.

The current size of the active file is tracked in memory: read from file
metadata when the file is opened, incremented by the byte count on each
successful flush, reset to zero after a rotation.

## 5. Decision on open

`RotatingFile::new` opens or creates the active file, then decides. `S` is the
size of the existing active file in bytes, `0` if it does not exist.

| Open strategy | Condition | Decision |
|---|---|---|
| `Append` | `S == 0` | append |
| `Append` | `0 < S < max_size` | append |
| `Append` | `S >= max_size` | rotate |
| `Rotate` | `S == 0` | append (nothing worth archiving) |
| `Rotate` | `S > 0` | rotate |

After that decision is applied, if the strategy is `KeepSome(n)` with `n >= 1`,
prune archives so at most `n` remain. A previous run under a different
configuration can leave more archives than the current setting allows, and
startup is where that gets corrected.

## 6. Decision on write

Applied inside `flush()`, before any bytes are written. `S` is the current
active size, `B` the buffered byte count, `M` is `max_size`.

| Condition | Decision |
|---|---|
| `S == 0` | append |
| `S + B <= M` | append |
| `S + B > M` | rotate, then append into the fresh file |

**The `S == 0` row is load-bearing.** A single buffered write larger than
`max_size` must be written whole to the empty file. Without this row an
oversized line rotates an empty file forever and never lands on disk.

## 7. Rotate resolution

A rotation does two things: it decides what happens to the current active file,
and which archives to delete. Then a fresh, empty active file is opened.

| Strategy | Active file becomes | Archives deleted |
|---|---|---|
| `KeepAll` | a new archive | none |
| `KeepSome(n)`, `n >= 1` | a new archive | oldest first, until exactly `n` remain **after** the new archive is counted |
| `KeepSome(0)` | deleted | none |
| `KeepOne` | deleted | none |

Two rules that must hold:

- **`KeepSome(0)` behaves exactly like `KeepOne`.** Do not compute
  `n - 1` anywhere: `n` is a `usize` and `0 - 1` underflows. Handle zero as its
  own case.
- **`KeepOne` and `KeepSome(0)` delete no archives.** They never create
  archives, so any that exist came from a previous configuration. Deleting
  files this configuration did not create is a destructive surprise. Leave
  them.

## 8. Archive naming

An archive is named:

```
<file_name>_<YYYY>-<MM>-<DD>_<HH>-<MM>-<SS>-<mmm>.log
```

Every component is zero-padded to fixed width: year 4, month/day/hour/minute/
second 2, milliseconds 3. The timestamp comes from
`TimezoneStrategy::get_now()`.

**Collisions.** If that path already exists, append `-1` before the extension,
then `-2`, and so on until the name is free:

```
app_2026-08-20_14-30-00-123.log
app_2026-08-20_14-30-00-123-1.log
app_2026-08-20_14-30-00-123-2.log
```

Never rename over an existing archive, and never delete one to make room. A
rotation must not be able to destroy an earlier rotation's output.

**Parsing and ordering.** A file in `dir` is an archive of this log if and only
if all of these hold:

- the name starts with `<file_name>_`
- the name ends with `.log`
- the text between them parses as `<stamp>` or `<stamp>-<seq>`, where `<stamp>`
  matches the fixed-width shape above and `<seq>` is a decimal integer
- it is not the active file `<file_name>.log`

Anything else in the directory is ignored. Strict parsing matters: with a
`file_name` of `app`, a file named `app_backup_2026-01-01.log` starts with
`app_` but is not ours, and deleting it would be data loss.

Order archives by the tuple `(stamp, seq)`, with a missing `seq` treated as
`0`. Do **not** sort on the raw filename: `-1` sorts before `.log` bytewise,
which would order a collision-suffixed archive as older than the unsuffixed
one it followed.

## 9. Errors

- Filesystem failures map to the existing `Error::Io`.
- Timestamp formatting failures map to the existing `Error::TimeFormat`.
- `policy.rs` and `naming.rs` return no errors. They are total functions over
  their inputs.
- **No `unwrap`, `expect`, `panic!`, or array indexing that can panic anywhere
  in these modules.** A logger that panics takes down the thing it is meant to
  be reporting on.
- `Write::flush` returns `std::io::Result<()>`, so wrap the crate error with
  `std::io::Error::other`, matching how `src/sink/file.rs` expects it.

## 10. Tests

Write the tests first. Unit tests live beside their module; integration tests
go in `tests/`.

**`policy.rs` -- pure, no filesystem, no clock:**

- append below threshold, at threshold, and over threshold
- `S == 0` with `B > max_size` appends and does not rotate
- rotate resolution for each of `KeepAll`, `KeepOne`, `KeepSome(0)`,
  `KeepSome(1)`, `KeepSome(3)`
- `KeepSome(n)` where existing archives are fewer than, equal to, and greater
  than `n`
- open decision: every row of the table in section 5

**`naming.rs` -- pure:**

- build then parse round-trips, with and without a `-N` suffix
- collision suffix selection given a set of taken names
- ordering across a day boundary and across a collision suffix
- rejects: the active file, a foreign file sharing the prefix, a malformed
  stamp, a file with no `.log` extension

**`tests/rotation.rs` -- integration, temp directories:**

- appending reopens the same file and preserves earlier content
- `FileOpenStrategy::Rotate` archives the previous session's file on open
- `FileOpenStrategy::Rotate` on an empty or absent file creates no archive
- `KeepAll` accumulates archives across several rotations
- `KeepSome(2)` leaves exactly two archives plus the active file
- `KeepOne` leaves only the active file
- content written across a rotation boundary is not lost or duplicated

**Regression gate:** the three existing tests in `tests/sink/file.rs` must pass
unmodified. Do not edit that file.

Note that `tests/lib.rs` is the single integration test target (`autotests =
false` in `Cargo.toml`, with an explicit `[[test]]` entry). A new
`tests/rotation.rs` must be declared as a module there, not left as a
standalone file.

## 11. Out of scope

`src/dtos/record_payload.rs`, `WEBVIEW_TARGET`, and the `log://log` event name
are also inherited from the original project, but they are wire-format
identifiers a consumer's JavaScript already depends on. Names and two-field
payload shapes are not protectable expression. They stay as they are.
