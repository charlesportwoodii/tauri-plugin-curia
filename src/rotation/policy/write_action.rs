/// What to do with a log file before bytes are written to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAction {
    /// Write into the file as it stands.
    Append,
    /// Rotate first, then write into the fresh file.
    Rotate,
}
