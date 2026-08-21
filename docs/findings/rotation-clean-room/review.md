# Clean-room review: log rotation

Date: 2026-08-20
Reviewer: the specifier (tainted party)
Subject: the replacement implementation in `src/rotation/` and `src/strategy/`

## Scope of this review

Conformance to `docs/superpowers/specs/2026-08-20-rotation-clean-room-design.md`
only. The result was **not** compared against the original code, by design. A
review that checked "does this match the original" would reintroduce the
influence the clean room exists to exclude.

## Verification, run by the reviewer rather than taken from the report

| Gate | Result |
|---|---|
| `cargo test` | 67 passed, 0 failed (46 unit, 21 integration) |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --all -- --check` | exit 0 |

The three pre-existing tests in `tests/sink/file.rs` pass unmodified, and
`git status` reports both that file and `src/sink/file.rs` with no working-tree
modification.

## Constraint checks

- `grep` for `unwrap`, `expect(`, `panic!`, and indexing across `src/rotation/`
  and `src/strategy/`: no matches.
- `grep` for `std::fs`, `fs::`, `now_utc`, `now_local`, `SystemTime`, `Instant`
  across `policy.rs` and `naming.rs`: no matches. The pure modules are pure.
- No dependency was added. The `Cargo.toml` change present in the working tree
  (`curia` from path to git, and a removed comment) was made externally at
  20:25, before the implementer was briefed, and is unrelated to this work.

## Conformance

Checked against the specification section by section. Sections 2, 4, 5, 6, 7,
8, 9, and 10 are all implemented as written.

Two specific points worth recording, because they are where a from-spec
implementation could most easily have gone wrong:

- Section 6's `S == 0` row is present and tested
  (`an_oversized_write_into_an_empty_file_appends_rather_than_rotating_forever`).
- Section 7's `KeepSome(0)` is matched as its own arm alongside `KeepOne`, with
  no subtraction anywhere in the strategy match. The underflow the spec forbids
  cannot occur.

## Specification defects the implementer found

Three, all genuine, all resolved correctly. They are defects in the
specification, not in the implementation.

1. **Section 9 contradicted itself** — it required timestamp formatting to map
   to `Error::TimeFormat` while also requiring `naming.rs` to be total.
   Resolved in favour of totality, rendering the stamp with `format!` width
   specifiers that cannot fail. This is the better of the two readings.
   Consequence: `Error::TimeFormat` is no longer produced by any code in the
   crate. The variant stays in the public `Error` enum, since removing it would
   be a breaking API change for no benefit. Accepted.

2. **Section 7 said only "oldest first"** for `KeepSome(n)` deletion. If the
   system clock moves backwards, the archive just created can sort oldest, and
   a literal reading would delete the rotation's own output. The implementer
   drew the deletion set only from archives present before the rename. This is
   correct and preserves section 8's guarantee. Accepted, and it is a better
   answer than the specification gave.

3. **Section 5's rows overlap when `max_size` is zero.** Row one is
   unconditional and wins; pinned by a test. Accepted.

## Reported risks

The implementer's risk list in `implementation.md` section 6 was reviewed. All
eight items are accurate. None blocks acceptance. The two worth carrying
forward:

- No test forces an I/O failure, so the `rename` and `remove_file` error paths
  and the `NotFound` tolerance are reasoned about but not exercised. Testing
  them needs a filesystem seam the specification did not ask for.
- The integration-level collision test is opportunistic: it relies on five
  rotations landing inside one millisecond. Deterministic coverage of the
  collision path exists in `naming.rs`'s unit tests, which is where it belongs.

## Verdict

**Accepted.** The implementation conforms to the specification, passes every
gate, and does not reproduce either of the two defects the specification
identified in the original.

## Addendum: post-review restructure (2026-08-20)

PR feedback asked for two BVC codebase rules to be applied: one struct per file,
and no free-floating functions. The rotation code was restructured to comply.
This was a mechanical reorganisation after the clean room closed. **No behaviour
changed**, and the specification was deliberately left as it was written, since
it is the provenance artifact the implementer worked from.

| Before | After |
|---|---|
| `rotation/policy.rs` (4 free fns, 3 types) | `rotation/policy/` -- `Policy` in `mod.rs`, one file each for `WriteAction`, `ActiveFileFate`, `RotationPlan` |
| `rotation/naming.rs` (7 free fns, 1 type) | `rotation/naming/` -- `Stamp` and `Archive`, one file each, all functions as associated functions |
| `rotation/writer.rs` (3 free fns) | `rotation/rotating_file.rs` -- named for its struct, the three helpers folded into `impl RotatingFile` |
| `commands.rs` held the `WebviewRecord` DTO | `dtos/webview_record.rs`; `commands.rs` keeps only the `#[tauri::command]` |

The bare `String` stamp became a `Stamp` newtype that owns its own shape
constant, rendering, and validation. That is the one change with any substance:
it added four unit tests and moved stamp validation off `Archive`. Test count
went from 67 to 71, all passing, with clippy and `fmt --check` clean.

Two free functions remain by agreement, both framework conventions the rules
already exempt in kind:

- `commands::log` -- a `#[tauri::command]` cannot hang off a struct. It holds no
  logic; the conversion lives on `WebviewRecord`.
- `lib::init` -- every Tauri plugin is mounted as `tauri_plugin_<name>::init()`.
  It delegates to `Plugin::init`.

## Task Completed
