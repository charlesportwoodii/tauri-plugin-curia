use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use tauri_plugin_curia::{FileOpenStrategy, RotatingFile, RotationStrategy, TimezoneStrategy};

const LOG: &str = "app";

/// A line of exactly eleven bytes once the newline is counted, so a `max_size`
/// of ten forces a rotation before every line but the first.
fn line(n: usize) -> String {
    format!("line-{n:05}")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rotation-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(
    dir: &Path,
    max_size: u64,
    rotation: RotationStrategy,
    file_open: FileOpenStrategy,
) -> RotatingFile {
    RotatingFile::new(
        dir,
        LOG.to_string(),
        max_size,
        rotation,
        TimezoneStrategy::UseUtc,
        file_open,
    )
    .unwrap()
}

fn write_lines(file: &mut RotatingFile, lines: &[String]) {
    for text in lines {
        writeln!(file, "{text}").unwrap();
        file.flush().unwrap();
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

fn archives(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .filter(|name| name != "app.log")
        .collect()
}

fn active(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("app.log")).unwrap()
}

/// Every line across the active file and every archive, in no particular order.
fn all_lines(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .flat_map(|name| {
            std::fs::read_to_string(dir.join(name))
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn touch_archive(dir: &Path, stamp: &str) {
    std::fs::write(dir.join(format!("app_{stamp}.log")), b"old\n").unwrap();
}

#[test]
fn appending_reopens_the_same_file_and_keeps_what_was_there() {
    let dir = scratch("append");

    let mut first = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut first, &[line(1)]);
    drop(first);

    let mut second = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut second, &[line(2)]);
    drop(second);

    assert_eq!(active(&dir).lines().collect::<Vec<_>>(), [line(1), line(2)]);
    assert!(archives(&dir).is_empty(), "{:?}", archives(&dir));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_with_rotate_archives_the_previous_session() {
    let dir = scratch("rotate-open");

    let mut first = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut first, &[line(1)]);
    drop(first);

    let second = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Rotate,
    );
    drop(second);

    let archived = archives(&dir);
    assert_eq!(archived.len(), 1, "{archived:?}");
    assert_eq!(active(&dir), "");
    let carried = std::fs::read_to_string(dir.join(&archived[0])).unwrap();
    assert_eq!(carried.lines().collect::<Vec<_>>(), [line(1)]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_with_rotate_on_an_absent_file_archives_nothing() {
    let dir = scratch("rotate-absent");

    let file = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Rotate,
    );
    drop(file);

    assert_eq!(archives(&dir), Vec::<String>::new());
    assert_eq!(active(&dir), "");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_with_rotate_on_an_empty_file_archives_nothing() {
    let dir = scratch("rotate-empty");

    let first = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    drop(first);
    assert_eq!(active(&dir), "");

    let second = open(
        &dir,
        1_000,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Rotate,
    );
    drop(second);

    assert_eq!(archives(&dir), Vec::<String>::new());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn keep_all_accumulates_one_archive_for_every_rotation() {
    let dir = scratch("keep-all");
    let lines: Vec<String> = (1..=4).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    let archived = archives(&dir);
    assert_eq!(archived.len(), 3, "{archived:?}");

    // The rotations happen inside the same millisecond, so this also proves the
    // collision suffix keeps them from overwriting one another.
    let distinct: HashSet<&String> = archived.iter().collect();
    assert_eq!(distinct.len(), 3, "{archived:?}");

    assert_eq!(active(&dir).lines().collect::<Vec<_>>(), [line(4)]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn keep_some_leaves_exactly_the_budgeted_archives_beside_the_active_file() {
    let dir = scratch("keep-some");
    let lines: Vec<String> = (1..=5).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepSome(2),
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    let archived = archives(&dir);
    assert_eq!(archived.len(), 2, "{archived:?}");
    assert_eq!(names(&dir).len(), 3, "{:?}", names(&dir));
    assert_eq!(active(&dir).lines().collect::<Vec<_>>(), [line(5)]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn keep_one_leaves_only_the_active_file() {
    let dir = scratch("keep-one");
    let lines: Vec<String> = (1..=4).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepOne,
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    assert_eq!(names(&dir), ["app.log"]);
    assert_eq!(active(&dir).lines().collect::<Vec<_>>(), [line(4)]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn keep_some_zero_behaves_the_same_way_as_keep_one() {
    let dir = scratch("keep-some-zero");
    let lines: Vec<String> = (1..=4).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepSome(0),
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    assert_eq!(names(&dir), ["app.log"]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn nothing_written_across_a_rotation_boundary_is_lost_or_duplicated() {
    let dir = scratch("boundary");
    let lines: Vec<String> = (1..=10).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    let mut landed = all_lines(&dir);
    landed.sort();
    let mut expected = lines.clone();
    expected.sort();

    assert_eq!(landed, expected);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_single_write_larger_than_the_limit_lands_whole_in_an_empty_file() {
    let dir = scratch("oversized");
    let long = "x".repeat(500);

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    writeln!(file, "{long}").unwrap();
    file.flush().unwrap();
    drop(file);

    assert_eq!(active(&dir).lines().collect::<Vec<_>>(), [long]);
    assert!(archives(&dir).is_empty(), "{:?}", archives(&dir));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn startup_prunes_archives_a_previous_configuration_left_behind() {
    let dir = scratch("startup-prune");
    for second in 1..=5 {
        touch_archive(&dir, &format!("2026-01-01_00-00-0{second}-000"));
    }

    let file = open(
        &dir,
        1_000,
        RotationStrategy::KeepSome(2),
        FileOpenStrategy::Append,
    );
    drop(file);

    assert_eq!(
        archives(&dir),
        [
            "app_2026-01-01_00-00-04-000.log",
            "app_2026-01-01_00-00-05-000.log"
        ]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn startup_leaves_archives_alone_under_a_strategy_that_creates_none() {
    let dir = scratch("startup-keep-one");
    for second in 1..=3 {
        touch_archive(&dir, &format!("2026-01-01_00-00-0{second}-000"));
    }

    let file = open(
        &dir,
        1_000,
        RotationStrategy::KeepOne,
        FileOpenStrategy::Append,
    );
    drop(file);

    assert_eq!(archives(&dir).len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn files_that_are_not_ours_are_never_deleted() {
    let dir = scratch("foreign");
    std::fs::write(dir.join("app_backup_2026-01-01.log"), b"precious\n").unwrap();
    std::fs::write(dir.join("other_2026-01-01_00-00-01-000.log"), b"precious\n").unwrap();
    std::fs::write(dir.join("notes.txt"), b"precious\n").unwrap();
    for second in 1..=3 {
        touch_archive(&dir, &format!("2026-01-01_00-00-0{second}-000"));
    }

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepSome(1),
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &[line(1), line(2)]);
    drop(file);

    let survivors = names(&dir);
    for kept in [
        "app_backup_2026-01-01.log",
        "other_2026-01-01_00-00-01-000.log",
        "notes.txt",
    ] {
        assert!(survivors.contains(&kept.to_string()), "{survivors:?}");
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_rotation_never_renames_over_an_existing_archive() {
    let dir = scratch("collision");
    let lines: Vec<String> = (1..=6).map(line).collect();

    let mut file = open(
        &dir,
        10,
        RotationStrategy::KeepAll,
        FileOpenStrategy::Append,
    );
    write_lines(&mut file, &lines);
    drop(file);

    // Five rotations inside one millisecond: every one of them has to find its
    // own name, and none of the earlier content may be overwritten.
    let archived = archives(&dir);
    assert_eq!(archived.len(), 5, "{archived:?}");

    let contents: HashSet<String> = archived
        .iter()
        .map(|name| std::fs::read_to_string(dir.join(name)).unwrap())
        .collect();
    assert_eq!(contents.len(), 5, "{contents:?}");

    std::fs::remove_dir_all(&dir).ok();
}
