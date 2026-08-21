/// What becomes of the active file during a rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveFileFate {
    /// Renamed to a new archive.
    Archive,
    /// Deleted. The strategy keeps no archives, so there is nowhere to put it.
    Discard,
}
