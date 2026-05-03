use crate::level::LogLevel;

pub struct Metadata {
    pub level: LogLevel,
    pub format_str: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub module_path: &'static str,
}
