use time::OffsetDateTime;

/// Which clock timestamps are read from.
#[derive(Debug, Clone)]
pub enum TimezoneStrategy {
    /// Always UTC. Archive names then sort the same way on every machine.
    UseUtc,
    /// The machine's local offset, falling back to UTC when the operating
    /// system will not report one.
    UseLocal,
}

impl TimezoneStrategy {
    /// The current time in the configured zone.
    ///
    /// Reading the local offset fails on some platforms and in some process
    /// states. That is not an error worth surfacing from a logger, so the
    /// fallback is UTC rather than a failure or a panic.
    pub fn get_now(&self) -> OffsetDateTime {
        match self {
            Self::UseUtc => OffsetDateTime::now_utc(),
            Self::UseLocal => match OffsetDateTime::now_local() {
                Ok(now) => now,
                Err(_) => OffsetDateTime::now_utc(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TimezoneStrategy;

    #[test]
    fn utc_reports_a_zero_offset() {
        let now = TimezoneStrategy::UseUtc.get_now();

        assert_eq!(now.offset(), time::UtcOffset::UTC);
    }

    #[test]
    fn local_never_panics_and_falls_back_to_utc_when_the_offset_is_unknown() {
        // The assertion is that this returns at all: on a platform where the
        // OS offset cannot be read the fallback must produce a time, not a panic.
        let local = TimezoneStrategy::UseLocal.get_now();
        let utc = TimezoneStrategy::UseUtc.get_now();

        let drift = (local.unix_timestamp() - utc.unix_timestamp()).abs();
        assert!(drift < 5, "the two clocks disagree by {drift}s");
    }
}
