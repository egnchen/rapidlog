use crate::level::LogLevel;

pub struct Metadata {
    pub level: LogLevel,
    pub format_str: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub module_path: &'static str,
}

impl Metadata {
    pub const fn new(
        level: LogLevel,
        format_str: &'static str,
        file: &'static str,
        line: u32,
        module_path: &'static str,
    ) -> Self {
        Self {
            level,
            format_str,
            file,
            line,
            module_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metadata() {
        let meta = Metadata::new(
            LogLevel::Info,
            "test fmt {}",
            file!(),
            line!(),
            module_path!(),
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
            let meta = Metadata::new(level, "", "", 0, "");
            assert_eq!(meta.level, level);
        }
    }
}
