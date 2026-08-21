# Rotation clean-room implementation

Date: 2026-08-20
Source of truth: `docs/superpowers/specs/2026-08-20-rotation-clean-room-design.md`

## 1. What was built

### `src/strategy/rotation.rs`

`RotationStrategy { KeepAll, KeepOne, KeepSome(usize) }`, deriving
`Debug, Clone, Serialize, Deserialize`. Unit tests pin the serialised forms
(`"KeepAll"`, `"KeepOne"`, `{"KeepSome":3}`) because the spec calls the variant
names consumer-visible configuration surface. Those tests are the guard against
a future rename slipping through.

### `src/strategy/file_open.rs`

`FileOpenStrategy { Append, Rotate }`, deriving `Debug, Clone, PartialEq`.

### `src/strategy/timezone.rs`

`TimezoneStrategy { UseUtc, UseLocal }`, deriving `Debug, Clone`. `get_now`
returns `OffsetDateTime::now_utc()` for `UseUtc`, and for `UseLocal` matches on
`OffsetDateTime::now_local()`, falling back to UTC on `Err`. Written as a
`match` rather than `unwrap_or_else` so that a grep for `unwrap` over these
modules returns nothing at all.

### `src/rotation/policy.rs` — pure decisions

No `std::fs`, no clock. Verified by grep (section 4 below).

Types:

- `WriteAction { Append, Rotate }`
- `ActiveFileFate { Archive, Discard }`
- `RotationPlan { active_file: ActiveFileFate, delete_oldest: usize }`

Functions:

- `on_open(&FileOpenStrategy, existing_size, max_size) -> WriteAction` — one arm
  per row of spec section 5.
- `on_write(current_size, buffered, max_size) -> WriteAction` — spec section 6.
  Uses `saturating_add` so `max_size == u64::MAX` cannot overflow in a debug
  build. `tests/sink/file.rs` passes exactly that value, so this is load
  bearing, not defensive decoration.
- `resolve_rotation(&RotationStrategy, archive_count) -> RotationPlan` — spec
  section 7. `KeepOne` and `KeepSome(0)` share one match arm, which is the
  structural way of guaranteeing they behave identically and that no `n - 1`
  underflow is reachable.
- `prune_on_open(&RotationStrategy, archive_count) -> usize` — the startup
  correction from spec section 5.

**Design decision.** The spec describes rotation resolution as a table but does
not name a return type. A struct with an explicit `ActiveFileFate` was chosen
over a bare `bool` so that a caller reading `plan.active_file` cannot misread
which way round the flag goes.

### `src/rotation/naming.rs` — pure names

No `std::fs`, no clock. The timestamp arrives as an argument.

- `active_name(file_name) -> String`
- `format_stamp(OffsetDateTime) -> String`
- `archive_name(file_name, stamp, Option<usize>) -> String`
- `parse_archive(file_name, name) -> Option<Archive>`
- `pick_free_name(file_name, stamp, &HashSet<String>) -> String`
- `Archive { stamp: String, seq: usize, name: String }`

**Design decisions:**

1. *The stamp shape is held as data.* `STAMP_SHAPE = "DDDD-DD-DD_DD-DD-DD-DDD"`,
   where `D` means "one decimal digit" and every other byte means itself.
   Validation zips the candidate against it. This gives the width check, the
   separator check and the length constant a single definition, and it needs no
   indexing, so no index can be out of bounds.

2. *Ordering is a derived `Ord` on field declaration order.* `Archive` declares
   `stamp` then `seq` then `name`, so the derive produces exactly the
   `(stamp, seq)` ordering the spec asks for. Stamps are fixed width, so
   comparing them as text compares them as instants — which is the whole reason
   the width is fixed. The spec's warning about sorting raw filenames is covered
   by a test asserting that `...-123.log` sorts before `...-123-1.log`, the
   opposite of the bytewise order.

3. *Collision search is bounded and total.* `pick_free_name` tries the
   unsuffixed name and then `1..=taken.len()`. That is `taken.len() + 1`
   distinct candidates against `taken.len()` taken names, so by pigeonhole at
   least one is free. The unreachable `None` arm returns the unsuffixed name
   rather than panicking.

4. *Parsing is strict and fails closed.* Everything `parse_archive` returns is a
   deletion candidate, so anything not matching exactly is rejected. Rejection
   tests cover the active file, a foreign file sharing the prefix, another log's
   archive, seven malformed stamps, four malformed sequence suffixes, and three
   wrong extensions.

### `src/rotation/writer.rs` — the only module that touches disk

`RotatingFile` holds `dir`, `file_name`, `max_size`, both strategies,
`file: Option<File>`, `current_size: u64` and `buffer: Vec<u8>`.

**Design decisions:**

1. *`file` is an `Option`.* A rotation sets it to `None` before renaming, which
   closes the handle. Two reasons: on Windows a rename can be refused while a
   handle is open, and on Unix the handle would follow the file into the archive
   and keep appending there. Reopening happens at the end of `rotate`, so the
   `None` window is internal to that one function.

2. *One `readdir` per rotation.* `scan()` returns both the archives ordered
   oldest first and the set of every name in the directory. The first drives
   pruning, the second drives collision avoidance. The set deliberately includes
   files that are not ours: the point is to avoid writing over anything at all,
   not just over our own archives.

3. *The just-created archive is never a deletion candidate.* `delete_oldest`
   only ever sees the archive list captured before the rename. See the ambiguity
   note in section 2 below.

4. *The buffer keeps its capacity across flushes.* `flush` destructures `self`
   for disjoint field borrows and calls `buffer.clear()` rather than
   `std::mem::take`. `FileSink`'s worker flushes after every single line, so a
   `take` would allocate once per log line.

5. *The buffer is cleared even when the write fails.* A partial `write_all` that
   errored may already have put some bytes on disk; replaying the whole buffer
   on the next flush would duplicate them. Losing the line is the lesser fault,
   and it is what keeps "not duplicated" true.

6. *`Drop` flushes.* This is an addition, not something the spec asks for. It
   follows `std::io::BufWriter`'s convention and stops a caller who forgets to
   flush from silently losing the tail. The result is discarded — there is
   nobody to report to at that point. Flag it if you would rather not have it;
   removing it does not affect any test, because `FileSink` always flushes.

7. *`NotFound` is tolerated on both `rename` and `remove_file`.* If the active
   file is deleted out from under a running process, the rotation continues
   rather than wedging the logger permanently.

8. *Non-UTF-8 directory entries are skipped.* Every name this crate writes is
   ASCII, so a name the platform will not hand back as UTF-8 is neither ours nor
   in our way. Directory entries that are not files are excluded from the
   archive list but still counted as taken names.

### `src/rotation/mod.rs`

`mod naming; mod policy; mod writer;` plus `pub use writer::RotatingFile;`.

### `src/lib.rs`

Two lines changed: `mod rotating_file;` became `mod rotation;` and
`pub use rotating_file::RotatingFile;` became `pub use rotation::RotatingFile;`.
The public path `tauri_plugin_curia::RotatingFile` is unchanged.

### `tests/rotation.rs`

Fourteen integration tests over temp directories, declared as `mod rotation;` in
`tests/lib.rs`. It covers all seven cases the spec lists, plus: `KeepSome(0)`
matching `KeepOne` end to end, an oversized single write landing whole, the
startup prune of a previous configuration's archives, the startup prune leaving
archives alone under `KeepOne`, foreign files surviving a pruning rotation, and
five rapid rotations producing five distinct archives with five distinct
contents.

## 2. Ambiguities found in the specification

### 2.1 Section 9 contradicts itself about timestamp formatting errors

Section 9 says both:

> Timestamp formatting failures map to the existing `Error::TimeFormat`.

and:

> `policy.rs` and `naming.rs` return no errors. They are total functions over
> their inputs.

Building an archive name is naming's job, so the two statements cannot both hold
if the stamp is rendered through a fallible `time` format description.

**How it was handled.** Totality won, since section 3 calls the purity of those
two modules "not negotiable" and section 10 asks for them to be tested without
any error plumbing. `format_stamp` renders the components with `format!` width
specifiers, which cannot fail. Consequence: **the rotation code never produces
`Error::TimeFormat`.** That variant is still declared in `src/error.rs` (which I
did not touch) and is still `#[from] time::error::Format`, so nothing broke — but
if the intent was for that variant to be reachable from rotation, this is the
decision to revisit.

I did not resolve this by looking for a reference implementation.

### 2.2 Section 7 does not say which files a `KeepSome(n)` rotation deletes

> oldest first, until exactly `n` remain **after** the new archive is counted

The count plainly includes the new archive. Which files get deleted is only
"oldest first". If the system clock has moved backwards, the archive just
created can sort as the oldest, and a literal reading would delete the rotation's
own output.

**How it was handled.** The deletion set is drawn only from archives that existed
before the rename. The remaining count is still exactly `n`, because
`delete_oldest = (count + 1) - n` never exceeds `count`. This also matches
section 8's "a rotation must not be able to destroy an earlier rotation's
output". Documented in a comment at the call site.

### 2.3 Section 5 rows overlap when `max_size` is zero

Rows two and three of the `Append` table are written as `0 < S < max_size` and
`S >= max_size`. Read as a single `S < max_size` comparison, an absent file
(`S == 0`) with `max_size == 0` would rotate, contradicting row one.

**How it was handled.** `on_open` matches `existing_size == 0` first, so row one
wins. A unit test
(`append_onto_a_missing_file_appends_even_when_the_limit_is_zero`) pins it. This
is not really a contradiction — row one is unconditional — but it is an easy
thing to get wrong by collapsing the rows.

Nothing else in the specification was unclear.

## 3. Clean-room confirmation

I did not read, fetch, search for, or reference any of the following:

- `tauri-plugin-log`, the `tauri-apps/plugins-workspace` repository, or any
  `fern`-based rotation code.
- crates.io, docs.rs, or GitHub, for those or anything else. No network tool was
  used at any point in this task.
- `third-party-licenses/` — I know from `git status` that the directory exists,
  and I did not open it or anything inside it.
- Any deleted blob. I ran no `git fsck`, `git cat-file`, `git show`,
  `git stash`, or reflog command. The only git commands run were
  `git status --porcelain` and `git diff` limited to files in the current
  working tree (`Cargo.toml`, `README.md`, `src/lib.rs`, `tests/lib.rs`,
  `src/sink/file.rs`, `tests/sink/file.rs`), which was to confirm I had not
  modified files I was told not to modify.
- No reference implementation of log rotation, from any source.

Files read: the specification, `src/sink/file.rs`, `src/error.rs`, `src/lib.rs`,
`src/strategy/mod.rs`, `src/format.rs`, `Cargo.toml`, `mise.toml`,
`tests/lib.rs`, `tests/sink/mod.rs`, `tests/sink/file.rs`. The `time` crate API
was used from prior knowledge; no documentation was fetched for it.

The implementation was written test-first: each module's tests were written and
run, the failure observed, and only then was the implementation added.

## 4. Constraint checks

- `src/sink/file.rs` compiles unchanged. `git status` reports it as `A ` (staged,
  no working-tree modification).
- `tests/sink/file.rs` passes unmodified, same `A ` status. All three of its
  tests are green in the run below.
- `tests/rotation.rs` is declared as `mod rotation;` in `tests/lib.rs`.
- No dependency was added to `Cargo.toml`. The only `Cargo.toml` diff in the
  tree predates this task (the `curia` path-to-git change and a removed
  comment); I did not touch the file. `time`, `serde` and `thiserror` were
  sufficient, and temp directories in `tests/rotation.rs` are built with
  `std::env::temp_dir()` in the same style as the existing
  `tests/sink/file.rs`, so no `tempfile` dependency was needed.
- No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` or
  panicking index anywhere in `src/rotation/` or `src/strategy/`. Verified by
  grep; the only hits were `#[test]` attributes and `buf: &[u8]`.
- `grep -n "std::fs\|OffsetDateTime::now\|SystemTime" src/rotation/policy.rs
  src/rotation/naming.rs` returns nothing.

## 5. Verification output

### `cargo test`

Exit code 0.

```
   Compiling tauri-plugin-curia v0.1.0 (C:\Users\charl\projects\tauri-plugin-curia)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.37s
     Running unittests src\lib.rs (target\debug\deps\tauri_plugin_curia-7807fd67e734f222.exe)

running 46 tests
test rotation::naming::tests::a_foreign_file_sharing_the_prefix_is_not_an_archive ... ok
test rotation::naming::tests::a_file_name_containing_a_separator_still_round_trips ... ok
test rotation::naming::tests::a_collision_suffixed_archive_sorts_after_the_one_it_followed ... ok
test rotation::naming::tests::a_file_without_the_log_extension_is_rejected ... ok
test rotation::naming::tests::a_free_name_is_used_unsuffixed ... ok
test rotation::naming::tests::a_malformed_stamp_is_rejected ... ok
test rotation::naming::tests::the_active_file_is_not_an_archive ... ok
test rotation::policy::tests::a_write_that_lands_exactly_on_the_limit_appends ... ok
test rotation::policy::tests::an_oversized_write_into_an_empty_file_appends_rather_than_rotating_forever ... ok
test rotation::naming::tests::an_unsuffixed_name_survives_a_build_and_parse_round_trip ... ok
test rotation::naming::tests::archives_order_oldest_first_across_a_day_boundary ... ok
test rotation::naming::tests::a_gap_in_the_suffixes_is_filled_rather_than_skipped ... ok
test rotation::naming::tests::every_stamp_component_is_padded_to_a_fixed_width ... ok
test rotation::naming::tests::milliseconds_are_truncated_rather_than_rounded_up ... ok
test rotation::naming::tests::a_malformed_collision_suffix_is_rejected ... ok
test rotation::naming::tests::suffixes_are_taken_in_order_and_never_reuse_an_existing_archive ... ok
test rotation::naming::tests::a_suffix_of_ten_sorts_after_a_suffix_of_two ... ok
test rotation::naming::tests::the_active_file_is_the_name_with_a_log_extension ... ok
test rotation::naming::tests::the_first_collision_takes_suffix_one ... ok
test rotation::policy::tests::a_write_that_stays_below_the_limit_appends ... ok
test rotation::policy::tests::a_write_that_would_pass_the_limit_rotates ... ok
test rotation::policy::tests::keep_some_three_counts_the_new_archive_towards_the_budget ... ok
test rotation::policy::tests::append_onto_a_full_file_rotates ... ok
test rotation::policy::tests::append_onto_a_missing_file_appends ... ok
test rotation::policy::tests::append_onto_a_missing_file_appends_even_when_the_limit_is_zero ... ok
test rotation::policy::tests::keep_all_archives_the_active_file_and_deletes_nothing ... ok
test rotation::policy::tests::keep_one_discards_the_active_file_and_leaves_foreign_archives_alone ... ok
test rotation::policy::tests::keep_some_deletes_nothing_when_fewer_archives_exist_than_the_budget ... ok
test rotation::policy::tests::keep_some_deletes_the_whole_overhang_when_a_previous_run_left_more ... ok
test rotation::naming::tests::an_archive_of_a_different_log_is_rejected ... ok
test rotation::policy::tests::append_onto_a_file_with_room_left_appends ... ok
test rotation::naming::tests::a_collision_suffixed_name_survives_a_build_and_parse_round_trip ... ok
test rotation::policy::tests::keep_some_zero_behaves_exactly_like_keep_one ... ok
test rotation::policy::tests::rotate_on_open_with_a_previous_session_on_disk_rotates ... ok
test rotation::policy::tests::keep_some_deletes_one_when_the_archives_already_fill_the_budget ... ok
test rotation::policy::tests::rotate_on_open_with_nothing_to_archive_appends ... ok
test rotation::policy::tests::keep_some_one_archives_and_deletes_every_older_archive ... ok
test rotation::policy::tests::startup_deletes_nothing_when_the_directory_is_within_budget ... ok
test rotation::policy::tests::startup_never_deletes_under_a_strategy_that_creates_no_archives ... ok
test rotation::policy::tests::startup_trims_an_over_full_directory_down_to_the_budget ... ok
test rotation::policy::tests::the_write_decision_does_not_overflow_on_an_unbounded_limit ... ok
test strategy::file_open::tests::the_two_variants_compare_by_value ... ok
test strategy::rotation::tests::a_config_file_deserialises_back_into_the_same_variant ... ok
test strategy::rotation::tests::the_variant_names_are_the_config_file_surface ... ok
test strategy::timezone::tests::utc_reports_a_zero_offset ... ok
test strategy::timezone::tests::local_never_panics_and_falls_back_to_utc_when_the_offset_is_unknown ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\lib.rs (target\debug\deps\lib-93ff9a24770bdd27.exe)

running 21 tests
test commands::a_missing_location_falls_back_to_the_bare_target ... ok
test commands::nested_values_and_types_survive_the_boundary ... ok
test commands::the_location_becomes_the_target ... ok
test macros::the_macros_are_reachable_through_the_plugin ... ok
test rotation::appending_reopens_the_same_file_and_keeps_what_was_there ... ok
test rotation::opening_with_rotate_on_an_absent_file_archives_nothing ... ok
test rotation::a_single_write_larger_than_the_limit_lands_whole_in_an_empty_file ... ok
test sink::file::the_formatter_output_is_what_lands_on_disk ... ok
test sink::file::the_sink_reports_its_registered_level ... ok
test rotation::opening_with_rotate_on_an_empty_file_archives_nothing ... ok
test rotation::opening_with_rotate_archives_the_previous_session ... ok
test sink::file::a_burst_past_the_queue_bound_never_blocks_and_never_loses_silently ... ok
test rotation::keep_one_leaves_only_the_active_file ... ok
test rotation::keep_some_zero_behaves_the_same_way_as_keep_one ... ok
test rotation::startup_leaves_archives_alone_under_a_strategy_that_creates_none ... ok
test rotation::keep_all_accumulates_one_archive_for_every_rotation ... ok
test rotation::startup_prunes_archives_a_previous_configuration_left_behind ... ok
test rotation::files_that_are_not_ours_are_never_deleted ... ok
test rotation::keep_some_leaves_exactly_the_budgeted_archives_beside_the_active_file ... ok
test rotation::a_rotation_never_renames_over_an_existing_archive ... ok
test rotation::nothing_written_across_a_rotation_boundary_is_lost_or_duplicated ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Doc-tests tauri_plugin_curia

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### `cargo clippy --all-targets -- -D warnings`

Exit code 0.

```
    Checking tauri-plugin-curia v0.1.0 (C:\Users\charl\projects\tauri-plugin-curia)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s
```

### `cargo fmt --all -- --check`

Exit code 0, no output.

```
```

## 6. What is risky or under-tested

1. **`Error::TimeFormat` is now unreachable from rotation.** See ambiguity 2.1.
   If a reviewer expects that variant to be produced, the stamp renderer has to
   change and `naming.rs` stops being total.

2. **Years outside `0..=9999` break the fixed width.** `format!("{:04}", year)`
   does not truncate, so a five-digit or negative year renders a stamp that
   `parse_archive` will then reject. The failure direction is safe — such a file
   is simply never recognised as an archive, so it is never pruned and never
   deleted — but the archive would be invisible to the budget. Not tested,
   because `time` without the `large-dates` feature cannot represent such a year
   and the system clock will not produce one.

3. **No test forces an I/O failure.** The `rename`, `remove_file` and
   `write_all` error paths, and the `NotFound` tolerance, are reasoned about but
   not exercised. Doing so needs either permission manipulation or a filesystem
   seam, and adding a seam would have meant putting I/O behind a trait that the
   spec's module layout does not describe.

4. **The collision path is exercised opportunistically, not deterministically,
   at the integration level.** `a_rotation_never_renames_over_an_existing_archive`
   drives five rotations back to back and asserts five distinct files with five
   distinct contents. On this machine those land inside one millisecond, so the
   `-N` suffix is what makes it pass. On a slow enough machine the stamps could
   differ and the test would pass without touching the collision path. The
   deterministic coverage is in `naming.rs`'s unit tests, which pass an explicit
   set of taken names.

5. **`Drop` swallows flush errors,** and swallows a rotation failure with them.
   That is inherent to `Drop`, but worth knowing.

6. **Nothing coordinates two processes sharing one log directory.** Two
   `RotatingFile` instances over the same `dir`/`file_name` will fight over the
   active file and over pruning. This is a property of the design the spec
   describes, not something introduced here, but it is not defended against and
   not tested.

7. **A crash between the rename and the reopen inside `rotate` leaves no active
   file.** The next `RotatingFile::new` creates it, so the window is
   self-healing, and no log content is lost — the archive is already in place.

8. **`current_size` is tracked in memory and never re-read.** If something else
   writes to the active file while the process runs, the rotation threshold
   drifts from the real file size. The spec specifies this tracking explicitly,
   so it is intended behaviour.

## Task Completed
