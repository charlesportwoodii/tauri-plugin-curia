//! Rotation decisions, with nothing underneath them.
//!
//! Every input arrives as an argument: no filesystem, no clock, no state. That
//! is what lets the whole decision table be tested without a temp directory,
//! and it keeps the interesting logic away from the code that deletes files.

mod active_file_fate;
mod rotation_plan;
mod write_action;

pub use active_file_fate::ActiveFileFate;
pub use rotation_plan::RotationPlan;
pub use write_action::WriteAction;

use crate::{FileOpenStrategy, RotationStrategy};

/// The rotation decisions, gathered in one place.
///
/// Every method is a total function of its arguments alone, which is why they
/// hang off a unit struct rather than off any state.
pub struct Policy;

impl Policy {
    /// The decision taken when the log file is first opened.
    ///
    /// `existing_size` is the size of the active file on disk, zero when it
    /// does not exist.
    pub fn on_open(
        file_open_strategy: &FileOpenStrategy,
        existing_size: u64,
        max_size: u64,
    ) -> WriteAction {
        match file_open_strategy {
            // An empty file is handled explicitly rather than by the comparison
            // below, which would rotate an absent file when `max_size` is zero.
            FileOpenStrategy::Append if existing_size == 0 => WriteAction::Append,
            FileOpenStrategy::Append if existing_size < max_size => WriteAction::Append,
            FileOpenStrategy::Append => WriteAction::Rotate,
            FileOpenStrategy::Rotate if existing_size == 0 => WriteAction::Append,
            FileOpenStrategy::Rotate => WriteAction::Rotate,
        }
    }

    /// The decision taken before a buffered write reaches the active file.
    ///
    /// `current_size` is what the active file already holds and `buffered` is
    /// what is about to be added to it.
    ///
    /// The empty-file case is load bearing: a single buffered write larger than
    /// `max_size` has to land somewhere, and rotating an already empty file
    /// would only produce empty archives while the line is never written at
    /// all.
    pub fn on_write(current_size: u64, buffered: u64, max_size: u64) -> WriteAction {
        if current_size == 0 {
            return WriteAction::Append;
        }

        if current_size.saturating_add(buffered) <= max_size {
            WriteAction::Append
        } else {
            WriteAction::Rotate
        }
    }

    /// Resolves one rotation against the configured strategy.
    ///
    /// `archive_count` is how many archives of this log already exist.
    pub fn resolve_rotation(
        rotation_strategy: &RotationStrategy,
        archive_count: usize,
    ) -> RotationPlan {
        match rotation_strategy {
            RotationStrategy::KeepAll => RotationPlan {
                active_file: ActiveFileFate::Archive,
                delete_oldest: 0,
            },

            // Neither of these ever creates an archive, so any archive present
            // came from a previous configuration. Deleting a file this
            // configuration did not write would be a destructive surprise.
            // Zero is matched on its own: `n - 1` on a `usize` would underflow.
            RotationStrategy::KeepOne | RotationStrategy::KeepSome(0) => RotationPlan {
                active_file: ActiveFileFate::Discard,
                delete_oldest: 0,
            },

            RotationStrategy::KeepSome(keep) => RotationPlan {
                active_file: ActiveFileFate::Archive,
                // The archive about to be created counts towards the budget.
                delete_oldest: archive_count.saturating_add(1).saturating_sub(*keep),
            },
        }
    }

    /// How many archives to delete at startup, oldest first.
    ///
    /// A previous run under a larger budget can leave more archives behind than
    /// the current setting allows, and startup is where that gets corrected.
    /// Strategies that create no archives prune nothing, for the same reason
    /// they delete nothing during a rotation.
    pub fn prune_on_open(rotation_strategy: &RotationStrategy, archive_count: usize) -> usize {
        match rotation_strategy {
            RotationStrategy::KeepSome(keep) if *keep >= 1 => archive_count.saturating_sub(*keep),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveFileFate, Policy, RotationPlan, WriteAction};
    use crate::{FileOpenStrategy, RotationStrategy};

    // --- the write decision ---------------------------------------------

    #[test]
    fn a_write_that_stays_below_the_limit_appends() {
        assert_eq!(Policy::on_write(10, 5, 100), WriteAction::Append);
    }

    #[test]
    fn a_write_that_lands_exactly_on_the_limit_appends() {
        assert_eq!(Policy::on_write(90, 10, 100), WriteAction::Append);
    }

    #[test]
    fn a_write_that_would_pass_the_limit_rotates() {
        assert_eq!(Policy::on_write(90, 11, 100), WriteAction::Rotate);
    }

    #[test]
    fn an_oversized_write_into_an_empty_file_appends_rather_than_rotating_forever() {
        assert_eq!(Policy::on_write(0, 5_000, 100), WriteAction::Append);
    }

    #[test]
    fn the_write_decision_does_not_overflow_on_an_unbounded_limit() {
        assert_eq!(
            Policy::on_write(u64::MAX - 1, 4_096, u64::MAX),
            WriteAction::Append
        );
    }

    // --- the open decision, one case per row of the specification table --

    #[test]
    fn append_onto_a_missing_file_appends() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Append, 0, 100),
            WriteAction::Append
        );
    }

    #[test]
    fn append_onto_a_file_with_room_left_appends() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Append, 50, 100),
            WriteAction::Append
        );
    }

    #[test]
    fn append_onto_a_full_file_rotates() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Append, 100, 100),
            WriteAction::Rotate
        );
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Append, 101, 100),
            WriteAction::Rotate
        );
    }

    #[test]
    fn append_onto_a_missing_file_appends_even_when_the_limit_is_zero() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Append, 0, 0),
            WriteAction::Append
        );
    }

    #[test]
    fn rotate_on_open_with_nothing_to_archive_appends() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Rotate, 0, 100),
            WriteAction::Append
        );
    }

    #[test]
    fn rotate_on_open_with_a_previous_session_on_disk_rotates() {
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Rotate, 1, 100),
            WriteAction::Rotate
        );
        assert_eq!(
            Policy::on_open(&FileOpenStrategy::Rotate, 9_999, 100),
            WriteAction::Rotate
        );
    }

    // --- rotate resolution ------------------------------------------------

    #[test]
    fn keep_all_archives_the_active_file_and_deletes_nothing() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepAll, 12),
            RotationPlan {
                active_file: ActiveFileFate::Archive,
                delete_oldest: 0
            }
        );
    }

    #[test]
    fn keep_one_discards_the_active_file_and_leaves_foreign_archives_alone() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepOne, 4),
            RotationPlan {
                active_file: ActiveFileFate::Discard,
                delete_oldest: 0
            }
        );
    }

    #[test]
    fn keep_some_zero_behaves_exactly_like_keep_one() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(0), 4),
            Policy::resolve_rotation(&RotationStrategy::KeepOne, 4)
        );
    }

    #[test]
    fn keep_some_one_archives_and_deletes_every_older_archive() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(1), 3),
            RotationPlan {
                active_file: ActiveFileFate::Archive,
                delete_oldest: 3
            }
        );
    }

    #[test]
    fn keep_some_three_counts_the_new_archive_towards_the_budget() {
        // Two on disk plus the one about to be created is three: nothing goes.
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(3), 2),
            RotationPlan {
                active_file: ActiveFileFate::Archive,
                delete_oldest: 0
            }
        );
    }

    #[test]
    fn keep_some_deletes_nothing_when_fewer_archives_exist_than_the_budget() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(5), 1).delete_oldest,
            0
        );
    }

    #[test]
    fn keep_some_deletes_one_when_the_archives_already_fill_the_budget() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(5), 5).delete_oldest,
            1
        );
    }

    #[test]
    fn keep_some_deletes_the_whole_overhang_when_a_previous_run_left_more() {
        assert_eq!(
            Policy::resolve_rotation(&RotationStrategy::KeepSome(2), 9).delete_oldest,
            8
        );
    }

    // --- the startup prune ------------------------------------------------

    #[test]
    fn startup_trims_an_over_full_directory_down_to_the_budget() {
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepSome(2), 5), 3);
    }

    #[test]
    fn startup_deletes_nothing_when_the_directory_is_within_budget() {
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepSome(2), 2), 0);
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepSome(2), 1), 0);
    }

    #[test]
    fn startup_never_deletes_under_a_strategy_that_creates_no_archives() {
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepAll, 9), 0);
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepOne, 9), 0);
        assert_eq!(Policy::prune_on_open(&RotationStrategy::KeepSome(0), 9), 0);
    }
}
