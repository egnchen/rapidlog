use crate::level::LogLevel;
use crate::metadata::Metadata;

/// Runtime filter evaluated by the backend worker before dispatching to sinks.
///
/// Filters are AND-combined: a message must pass ALL filters on its logger
/// to be delivered. Filters have access to the metadata and raw argument payload.
pub trait Filter: Send + Sync {
    /// Return `true` if the message should be delivered to sinks.
    fn accept(&self, metadata: &Metadata, args_raw: &[u8]) -> bool;
}

/// Filters messages at or above a minimum severity level.
///
/// Example: `LevelFilter::new(LogLevel::Warning)` allows Warning, Error, and
/// Critical messages, but blocks Info and below.
pub struct LevelFilter {
    level: LogLevel,
}

impl LevelFilter {
    pub fn new(level: LogLevel) -> Self {
        Self { level }
    }
}

impl Filter for LevelFilter {
    fn accept(&self, metadata: &Metadata, _args_raw: &[u8]) -> bool {
        metadata.level >= self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_schemas() -> &'static [u8] {
        &[]
    }

    #[test]
    fn level_filter_accepts_same() {
        let filter = LevelFilter::new(LogLevel::Warning);
        let meta = Metadata::new(
            LogLevel::Warning,
            "test",
            "f.rs",
            1,
            "mod",
            empty_schemas,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        );
        assert!(filter.accept(&meta, &[]));
    }

    #[test]
    fn level_filter_rejects_lower() {
        let filter = LevelFilter::new(LogLevel::Error);
        let meta = Metadata::new(
            LogLevel::Warning,
            "test",
            "f.rs",
            1,
            "mod",
            empty_schemas,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        );
        assert!(!filter.accept(&meta, &[]));
    }

    #[test]
    fn level_filter_accepts_higher() {
        let filter = LevelFilter::new(LogLevel::Info);
        let meta = Metadata::new(
            LogLevel::Critical,
            "test",
            "f.rs",
            1,
            "mod",
            empty_schemas,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        );
        assert!(filter.accept(&meta, &[]));
    }
}
