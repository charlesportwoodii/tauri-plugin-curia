use std::collections::HashSet;

use super::Stamp;

/// What separates the log name from the timestamp that follows it.
const SEPARATOR: char = '_';

/// What separates the timestamp from a collision sequence number.
const SEQUENCE_SEPARATOR: char = '-';

/// A recognised archive of one particular log.
///
/// The field order is the sort order. Stamps are fixed width, so comparing them
/// compares the instants they stand for, and the sequence number breaks ties
/// between archives rotated within the same millisecond. Sorting raw file names
/// would not work: a collision suffix starts with a character that sorts before
/// the dot of the extension, which would place a later archive first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Archive {
    /// When the archive was rotated out.
    pub stamp: Stamp,
    /// The collision sequence number, zero when the name carries none.
    pub seq: usize,
    /// The file name as it appears in the directory.
    pub name: String,
}

impl Archive {
    /// The extension carried by the active file and by every archive.
    pub const EXTENSION: &'static str = ".log";

    /// Builds an archive name from a stamp and an optional collision sequence
    /// number.
    pub fn name_for(file_name: &str, stamp: &Stamp, seq: Option<usize>) -> String {
        let stamp = stamp.as_str();
        let extension = Self::EXTENSION;

        match seq {
            Some(seq) => {
                format!("{file_name}{SEPARATOR}{stamp}{SEQUENCE_SEPARATOR}{seq}{extension}")
            }
            None => format!("{file_name}{SEPARATOR}{stamp}{extension}"),
        }
    }

    /// Recognises an archive of `file_name`, or returns `None`.
    ///
    /// The parse is deliberately strict. Everything this returns is a candidate
    /// for deletion, so a file that merely resembles an archive -- a foreign
    /// file sharing the prefix, say -- has to be rejected rather than guessed
    /// at. The active file is rejected too: it carries no separator after the
    /// log name.
    pub fn parse(file_name: &str, name: &str) -> Option<Self> {
        let body = name
            .strip_prefix(file_name)?
            .strip_suffix(Self::EXTENSION)?
            .strip_prefix(SEPARATOR)?;

        let (stamp, tail) = body.split_at_checked(Stamp::WIDTH)?;
        let stamp = Stamp::parse(stamp)?;

        let seq = match tail.is_empty() {
            true => 0,
            false => Self::parse_sequence(tail)?,
        };

        Some(Self {
            stamp,
            seq,
            name: name.to_string(),
        })
    }

    /// Chooses a free archive name for `stamp`, given every name already
    /// present in the directory.
    ///
    /// A rotation must never rename over an earlier rotation's output, so the
    /// unsuffixed name is tried first and successive sequence numbers after it.
    /// The search is bounded and always succeeds: among the unsuffixed name and
    /// the first `taken.len()` suffixed ones there are more candidates than
    /// taken names, so at least one of them is free.
    pub fn pick_free_name(file_name: &str, stamp: &Stamp, taken: &HashSet<String>) -> String {
        let suffixed = (1..=taken.len()).map(|seq| Self::name_for(file_name, stamp, Some(seq)));

        let free = std::iter::once(Self::name_for(file_name, stamp, None))
            .chain(suffixed)
            .find(|candidate| !taken.contains(candidate));

        match free {
            Some(name) => name,
            // Unreachable by the counting argument above, and not worth a panic.
            None => Self::name_for(file_name, stamp, None),
        }
    }

    /// Reads the `-N` that follows a stamp. Anything that is not a separator
    /// followed by at least one decimal digit is not an archive of ours.
    fn parse_sequence(tail: &str) -> Option<usize> {
        let digits = tail.strip_prefix(SEQUENCE_SEPARATOR)?;

        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }

        digits.parse::<usize>().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use time::macros::datetime;

    use super::{Archive, Stamp};

    fn stamp() -> Stamp {
        Stamp::render(datetime!(2026-08-20 14:30:00.123 UTC))
    }

    fn taken(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    // --- round trips -------------------------------------------------------

    #[test]
    fn an_unsuffixed_name_survives_a_build_and_parse_round_trip() {
        let name = Archive::name_for("app", &stamp(), None);

        assert_eq!(name, "app_2026-08-20_14-30-00-123.log");
        assert_eq!(
            Archive::parse("app", &name),
            Some(Archive {
                stamp: stamp(),
                seq: 0,
                name,
            })
        );
    }

    #[test]
    fn a_collision_suffixed_name_survives_a_build_and_parse_round_trip() {
        let name = Archive::name_for("app", &stamp(), Some(2));

        assert_eq!(name, "app_2026-08-20_14-30-00-123-2.log");
        assert_eq!(Archive::parse("app", &name).map(|a| a.seq), Some(2));
    }

    #[test]
    fn a_file_name_containing_a_separator_still_round_trips() {
        let name = Archive::name_for("my_app", &stamp(), None);

        assert_eq!(Archive::parse("my_app", &name).map(|a| a.seq), Some(0));
    }

    // --- collisions --------------------------------------------------------

    #[test]
    fn a_free_name_is_used_unsuffixed() {
        let chosen = Archive::pick_free_name("app", &stamp(), &taken(&["other.log"]));

        assert_eq!(chosen, "app_2026-08-20_14-30-00-123.log");
    }

    #[test]
    fn the_first_collision_takes_suffix_one() {
        let chosen = Archive::pick_free_name(
            "app",
            &stamp(),
            &taken(&["app_2026-08-20_14-30-00-123.log"]),
        );

        assert_eq!(chosen, "app_2026-08-20_14-30-00-123-1.log");
    }

    #[test]
    fn suffixes_are_taken_in_order_and_never_reuse_an_existing_archive() {
        let chosen = Archive::pick_free_name(
            "app",
            &stamp(),
            &taken(&[
                "app_2026-08-20_14-30-00-123.log",
                "app_2026-08-20_14-30-00-123-1.log",
                "app_2026-08-20_14-30-00-123-2.log",
            ]),
        );

        assert_eq!(chosen, "app_2026-08-20_14-30-00-123-3.log");
    }

    #[test]
    fn a_gap_in_the_suffixes_is_filled_rather_than_skipped() {
        let chosen = Archive::pick_free_name(
            "app",
            &stamp(),
            &taken(&[
                "app_2026-08-20_14-30-00-123.log",
                "app_2026-08-20_14-30-00-123-2.log",
            ]),
        );

        assert_eq!(chosen, "app_2026-08-20_14-30-00-123-1.log");
    }

    // --- ordering ----------------------------------------------------------

    fn sorted(file_name: &str, names: &[&str]) -> Vec<String> {
        let mut archives: Vec<Archive> = names
            .iter()
            .filter_map(|n| Archive::parse(file_name, n))
            .collect();
        archives.sort();
        archives.into_iter().map(|a| a.name).collect()
    }

    #[test]
    fn archives_order_oldest_first_across_a_day_boundary() {
        let order = sorted(
            "app",
            &[
                "app_2026-08-21_00-00-00-000.log",
                "app_2026-08-20_23-59-59-999.log",
                "app_2026-07-31_12-00-00-000.log",
            ],
        );

        assert_eq!(
            order,
            vec![
                "app_2026-07-31_12-00-00-000.log",
                "app_2026-08-20_23-59-59-999.log",
                "app_2026-08-21_00-00-00-000.log",
            ]
        );
    }

    #[test]
    fn a_collision_suffixed_archive_sorts_after_the_one_it_followed() {
        // Bytewise the filenames sort the other way round: the dash of a
        // suffix precedes the dot of the extension.
        let order = sorted(
            "app",
            &[
                "app_2026-08-20_14-30-00-123-2.log",
                "app_2026-08-20_14-30-00-123-1.log",
                "app_2026-08-20_14-30-00-123.log",
            ],
        );

        assert_eq!(
            order,
            vec![
                "app_2026-08-20_14-30-00-123.log",
                "app_2026-08-20_14-30-00-123-1.log",
                "app_2026-08-20_14-30-00-123-2.log",
            ]
        );
    }

    #[test]
    fn a_suffix_of_ten_sorts_after_a_suffix_of_two() {
        let order = sorted(
            "app",
            &[
                "app_2026-08-20_14-30-00-123-10.log",
                "app_2026-08-20_14-30-00-123-2.log",
            ],
        );

        assert_eq!(
            order,
            vec![
                "app_2026-08-20_14-30-00-123-2.log",
                "app_2026-08-20_14-30-00-123-10.log",
            ]
        );
    }

    // --- rejection ---------------------------------------------------------

    #[test]
    fn the_active_file_is_not_an_archive() {
        assert_eq!(Archive::parse("app", "app.log"), None);
    }

    #[test]
    fn a_foreign_file_sharing_the_prefix_is_not_an_archive() {
        // Deleting this because it starts with the prefix would be data loss.
        assert_eq!(Archive::parse("app", "app_backup_2026-01-01.log"), None);
    }

    #[test]
    fn an_archive_of_a_different_log_is_rejected() {
        assert_eq!(
            Archive::parse("app", "other_2026-08-20_14-30-00-123.log"),
            None
        );
        assert_eq!(
            Archive::parse("app", "app2_2026-08-20_14-30-00-123.log"),
            None
        );
    }

    #[test]
    fn a_malformed_stamp_is_rejected() {
        for name in [
            "app_2026-8-20_14-30-00-123.log",   // month not padded
            "app_2026-08-20_14-30-00-12.log",   // milliseconds too short
            "app_2026-08-20-14-30-00-123.log",  // wrong separator
            "app_20260820_143000123.log",       // no separators at all
            "app_xxxx-xx-xx_xx-xx-xx-xxx.log",  // not digits
            "app_2026-08-20_14-30-00-123x.log", // trailing rubbish
            "app_.log",                         // nothing at all
        ] {
            assert_eq!(Archive::parse("app", name), None, "accepted {name}");
        }
    }

    #[test]
    fn a_malformed_collision_suffix_is_rejected() {
        for name in [
            "app_2026-08-20_14-30-00-123-.log",   // empty suffix
            "app_2026-08-20_14-30-00-123-x.log",  // not a number
            "app_2026-08-20_14-30-00-123--1.log", // not a number either
            "app_2026-08-20_14-30-00-123_1.log",  // wrong separator
        ] {
            assert_eq!(Archive::parse("app", name), None, "accepted {name}");
        }
    }

    #[test]
    fn a_file_without_the_log_extension_is_rejected() {
        assert_eq!(Archive::parse("app", "app_2026-08-20_14-30-00-123"), None);
        assert_eq!(
            Archive::parse("app", "app_2026-08-20_14-30-00-123.txt"),
            None
        );
        assert_eq!(
            Archive::parse("app", "app_2026-08-20_14-30-00-123.log.gz"),
            None
        );
    }
}
