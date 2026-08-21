use serde::{Deserialize, Serialize};

/// How many archived log files a rotation is allowed to leave behind.
///
/// This type is part of the configuration surface: the variant names below are
/// what a consumer writes in a config file, so they are not free to change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationStrategy {
    /// Archive every rotation and delete nothing. The directory grows without
    /// bound, which is what you want when an operator collects the files.
    KeepAll,
    /// Keep only the active file. A rotation discards the file it rotates and
    /// creates no archive.
    KeepOne,
    /// Keep at most this many archives beside the active file. Zero behaves
    /// exactly like [`RotationStrategy::KeepOne`].
    KeepSome(usize),
}

#[cfg(test)]
mod tests {
    use super::RotationStrategy;

    #[test]
    fn the_variant_names_are_the_config_file_surface() {
        let keep_all = serde_json::to_string(&RotationStrategy::KeepAll);
        let keep_one = serde_json::to_string(&RotationStrategy::KeepOne);
        let keep_some = serde_json::to_string(&RotationStrategy::KeepSome(3));

        assert_eq!(keep_all.ok(), Some("\"KeepAll\"".to_string()));
        assert_eq!(keep_one.ok(), Some("\"KeepOne\"".to_string()));
        assert_eq!(keep_some.ok(), Some("{\"KeepSome\":3}".to_string()));
    }

    #[test]
    fn a_config_file_deserialises_back_into_the_same_variant() {
        let parsed: Result<RotationStrategy, _> = serde_json::from_str("{\"KeepSome\":7}");

        assert!(matches!(parsed, Ok(RotationStrategy::KeepSome(7))));
    }
}
