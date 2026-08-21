use super::ActiveFileFate;

/// The complete outcome of one rotation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPlan {
    /// Whether the active file is kept as an archive or thrown away.
    pub active_file: ActiveFileFate,
    /// How many of the existing archives to delete, oldest first.
    pub delete_oldest: usize,
}
