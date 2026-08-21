use time::OffsetDateTime;

/// The shape of a rendered timestamp, one character per character of the real
/// thing: `D` stands for a decimal digit, everything else for itself.
///
/// Holding the shape as data keeps the width check, the separator check and the
/// renderer honest about the same layout.
const SHAPE: &str = "DDDD-DD-DD_DD-DD-DD-DDD";

/// The timestamp carried by an archive name.
///
/// Fixed width in every component, which is what lets one stamp be compared
/// against another as plain text and still order as an instant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp(String);

impl Stamp {
    /// How many characters a rendered stamp occupies.
    pub const WIDTH: usize = SHAPE.len();

    /// Renders `now` as a fixed-width timestamp.
    ///
    /// Every component is padded to its width in [`SHAPE`], which is what makes
    /// the result both parseable and sortable as plain text. Width specifiers
    /// cannot fail, so this is total and the caller needs no error path.
    pub fn render(now: OffsetDateTime) -> Self {
        Self(format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}-{:03}",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.millisecond(),
        ))
    }

    /// Reads a stamp back, or returns `None` when `text` is not one.
    pub fn parse(text: &str) -> Option<Self> {
        match Self::matches_shape(text) {
            true => Some(Self(text.to_string())),
            false => None,
        }
    }

    /// The stamp as it appears inside a file name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `text` matches [`SHAPE`] exactly.
    fn matches_shape(text: &str) -> bool {
        text.len() == SHAPE.len()
            && text
                .bytes()
                .zip(SHAPE.bytes())
                .all(|(actual, expected)| match expected {
                    b'D' => actual.is_ascii_digit(),
                    separator => actual == separator,
                })
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::Stamp;

    #[test]
    fn every_stamp_component_is_padded_to_a_fixed_width() {
        let stamp = Stamp::render(datetime!(2026-01-02 03:04:05.006 UTC));

        assert_eq!(stamp.as_str(), "2026-01-02_03-04-05-006");
    }

    #[test]
    fn milliseconds_are_truncated_rather_than_rounded_up() {
        let stamp = Stamp::render(datetime!(2026-08-20 14:30:00.123999 UTC));

        assert_eq!(stamp.as_str(), "2026-08-20_14-30-00-123");
    }

    #[test]
    fn a_rendered_stamp_is_exactly_as_wide_as_the_shape() {
        let stamp = Stamp::render(datetime!(2026-08-20 14:30:00.123 UTC));

        assert_eq!(stamp.as_str().len(), Stamp::WIDTH);
    }

    #[test]
    fn a_rendered_stamp_parses_back() {
        let stamp = Stamp::render(datetime!(2026-08-20 14:30:00.123 UTC));

        assert_eq!(Stamp::parse(stamp.as_str()), Some(stamp));
    }

    #[test]
    fn a_malformed_stamp_is_rejected() {
        for text in [
            "2026-8-20_14-30-00-123",   // month not padded
            "2026-08-20_14-30-00-12",   // milliseconds too short
            "2026-08-20-14-30-00-123",  // wrong separator
            "20260820_143000123",       // no separators at all
            "xxxx-xx-xx_xx-xx-xx-xxx",  // not digits
            "2026-08-20_14-30-00-123x", // too long
            "",                         // nothing at all
        ] {
            assert_eq!(Stamp::parse(text), None, "accepted {text}");
        }
    }

    #[test]
    fn stamps_order_as_instants_across_a_day_boundary() {
        let earlier = Stamp::render(datetime!(2026-08-20 23:59:59.999 UTC));
        let later = Stamp::render(datetime!(2026-08-21 00:00:00.000 UTC));

        assert!(earlier < later);
    }
}
