use crate::level::LogLevel;

/// Returns an empty string tables list. Used as default for `Metadata::string_tables_provider`
/// when no args implement `HasStringTable`.
pub fn empty_string_tables_provider() -> &'static [&'static [u8]] {
    &[]
}

/// Returns an empty user formatters list. Used as default for `Metadata::user_formatters_provider`
/// when no user formatters are registered.
pub fn empty_user_formatters_provider() -> &'static [crate::arg::UserFormatter] {
    &[]
}

pub struct Metadata {
    pub level: LogLevel,
    pub format_str: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub module_path: &'static str,
    pub schema_provider: fn() -> &'static [u8],
    pub string_tables_provider: fn() -> &'static [&'static [u8]],
    pub user_formatters_provider: fn() -> &'static [crate::arg::UserFormatter],
}

impl Metadata {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        level: LogLevel,
        format_str: &'static str,
        file: &'static str,
        line: u32,
        module_path: &'static str,
        schema_provider: fn() -> &'static [u8],
        string_tables_provider: fn() -> &'static [&'static [u8]],
        user_formatters_provider: fn() -> &'static [crate::arg::UserFormatter],
    ) -> Self {
        Self {
            level,
            format_str,
            file,
            line,
            module_path,
            schema_provider,
            string_tables_provider,
            user_formatters_provider,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_schemas() -> &'static [u8] {
        &[]
    }

    #[test]
    fn new_metadata() {
        let meta = Metadata::new(
            LogLevel::Info,
            "test fmt {}",
            file!(),
            line!(),
            module_path!(),
            empty_schemas,
            empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        );

        assert_eq!(meta.level, LogLevel::Info);
        assert_eq!(meta.format_str, "test fmt {}");
        assert!(meta.file.contains("metadata.rs"));
        assert!(meta.line > 0);
        assert!(meta.module_path.contains("metadata"));
    }

    #[test]
    fn metadata_per_level() {
        for level in [
            LogLevel::TraceL3,
            LogLevel::TraceL2,
            LogLevel::TraceL1,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
            LogLevel::Critical,
        ] {
            let meta = Metadata::new(
                level,
                "",
                "",
                0,
                "",
                empty_schemas,
                empty_string_tables_provider,
                crate::metadata::empty_user_formatters_provider,
            );
            assert_eq!(meta.level, level);
        }
    }
}
