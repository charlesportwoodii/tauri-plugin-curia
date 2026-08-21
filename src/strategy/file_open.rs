/// What happens to an existing log file when the plugin starts.
#[derive(Debug, Clone, PartialEq)]
pub enum FileOpenStrategy {
    /// Continue the previous session's file, as long as it still has room.
    Append,
    /// Start each session in a fresh file, archiving whatever the last session
    /// left behind.
    Rotate,
}

#[cfg(test)]
mod tests {
    use super::FileOpenStrategy;

    #[test]
    fn the_two_variants_compare_by_value() {
        assert_eq!(FileOpenStrategy::Append, FileOpenStrategy::Append.clone());
        assert_ne!(FileOpenStrategy::Append, FileOpenStrategy::Rotate);
    }
}
