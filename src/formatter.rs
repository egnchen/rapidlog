use crate::level::LogLevel;

#[derive(Debug, Clone)]
enum Segment {
    Literal(String),
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    WholeSecs,
    FracSec,
    Level,
    File,
    Line,
    Body,
}

/// A pattern-based log message formatter.
///
/// Parses a format string containing `%`-prefixed specifiers. Unknown
/// specifiers are rendered literally. When no formatter is supplied
/// to [`BackendOptions`](crate::BackendOptions), a default format is used.
///
/// # Specifiers
///
/// | Spec | Output |
/// |------|--------|
/// | `%Y` | Year (4 digits) |
/// | `%m` | Month (01–12) |
/// | `%d` | Day (01–31) |
/// | `%H` | Hour (00–23) |
/// | `%M` | Minute (00–59) |
/// | `%S` | Second (00–59) |
/// | `%s` | Whole seconds (Unix timestamp) |
/// | `%f` | Fractional seconds (9 digits) |
/// | `%l` | Log level (e.g. `Info`, `Error`) |
/// | `%F` | Source file path |
/// | `%L` | Source line number |
/// | `%v` | Formatted message body |
pub struct PatternFormatter {
    segments: Vec<Segment>,
}

/// Input data for [`PatternFormatter::format`].
///
/// Constructed by the backend worker from a decoded log message.
pub struct FormattedRecord<'a> {
    pub timestamp_secs: u64,
    pub timestamp_nanos: u32,
    pub level: &'a LogLevel,
    pub file: &'a str,
    pub line: u32,
    pub body: &'a str,
}

impl PatternFormatter {
    pub fn new(pattern: &str) -> Self {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '%' && i + 1 < chars.len() {
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                match chars[i + 1] {
                    'Y' => segments.push(Segment::Year),
                    'm' => segments.push(Segment::Month),
                    'd' => segments.push(Segment::Day),
                    'H' => segments.push(Segment::Hour),
                    'M' => segments.push(Segment::Minute),
                    'S' => segments.push(Segment::Second),
                    's' => segments.push(Segment::WholeSecs),
                    'f' => segments.push(Segment::FracSec),
                    'l' => segments.push(Segment::Level),
                    'F' => segments.push(Segment::File),
                    'L' => segments.push(Segment::Line),
                    'v' => segments.push(Segment::Body),
                    other => {
                        literal.push('%');
                        literal.push(other);
                    }
                }
                i += 2;
            } else {
                literal.push(chars[i]);
                i += 1;
            }
        }
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }
        Self { segments }
    }

    pub fn format(&self, record: &FormattedRecord<'_>) -> String {
        let odt = time::OffsetDateTime::from_unix_timestamp(record.timestamp_secs as i64).ok();
        let mut result = String::new();
        for seg in &self.segments {
            match seg {
                Segment::Literal(s) => result.push_str(s),
                Segment::Year => {
                    if let Some(ref odt) = odt {
                        let _ = std::fmt::write(&mut result, format_args!("{:04}", odt.year()));
                    } else {
                        result.push_str("????");
                    }
                }
                Segment::Month => {
                    if let Some(ref odt) = odt {
                        let _ =
                            std::fmt::write(&mut result, format_args!("{:02}", odt.month() as u8));
                    } else {
                        result.push_str("??");
                    }
                }
                Segment::Day => {
                    if let Some(ref odt) = odt {
                        let _ = std::fmt::write(&mut result, format_args!("{:02}", odt.day()));
                    } else {
                        result.push_str("??");
                    }
                }
                Segment::Hour => {
                    if let Some(ref odt) = odt {
                        let _ = std::fmt::write(&mut result, format_args!("{:02}", odt.hour()));
                    } else {
                        result.push_str("??");
                    }
                }
                Segment::Minute => {
                    if let Some(ref odt) = odt {
                        let _ = std::fmt::write(&mut result, format_args!("{:02}", odt.minute()));
                    } else {
                        result.push_str("??");
                    }
                }
                Segment::Second => {
                    if let Some(ref odt) = odt {
                        let _ = std::fmt::write(&mut result, format_args!("{:02}", odt.second()));
                    } else {
                        result.push_str("??");
                    }
                }
                Segment::WholeSecs => {
                    let _ = std::fmt::write(&mut result, format_args!("{}", record.timestamp_secs));
                }
                Segment::FracSec => {
                    let _ =
                        std::fmt::write(&mut result, format_args!("{:09}", record.timestamp_nanos));
                }
                Segment::Level => {
                    let _ = std::fmt::write(&mut result, format_args!("{:?}", record.level));
                }
                Segment::File => result.push_str(record.file),
                Segment::Line => {
                    let _ = std::fmt::write(&mut result, format_args!("{}", record.line));
                }
                Segment::Body => result.push_str(record.body),
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::LogLevel;

    fn make_record(body: &str) -> FormattedRecord<'_> {
        FormattedRecord {
            timestamp_secs: 1_700_000_001,
            timestamp_nanos: 123_456_789,
            level: &LogLevel::Warning,
            file: "src/main.rs",
            line: 42,
            body,
        }
    }

    #[test]
    fn default_pattern_matches_current_format() {
        let fmt = PatternFormatter::new("[%s.%f] [%l] %F:%L %v");
        let record = make_record("test body");
        let output = fmt.format(&record);
        assert!(output.contains("[1700000001.123456789]"));
        assert!(output.contains("[Warning]"));
        assert!(output.contains("src/main.rs:42"));
        assert!(output.contains("test body"));
    }

    #[test]
    fn custom_date_pattern() {
        let fmt = PatternFormatter::new("%Y-%m-%d %H:%M:%S");
        let record = make_record("ignored");
        let output = fmt.format(&record);
        assert!(output.starts_with("202"));
        assert_eq!(output.len(), 19);
    }

    #[test]
    fn level_specifier() {
        let fmt = PatternFormatter::new("%l");
        let output = fmt.format(&make_record(""));
        assert_eq!(output, "Warning");
    }

    #[test]
    fn file_and_line_specifiers() {
        let fmt = PatternFormatter::new("%F:%L");
        let output = fmt.format(&make_record(""));
        assert_eq!(output, "src/main.rs:42");
    }

    #[test]
    fn body_specifier() {
        let fmt = PatternFormatter::new("%v");
        let output = fmt.format(&make_record("hello world"));
        assert_eq!(output, "hello world");
    }

    #[test]
    fn literal_text() {
        let fmt = PatternFormatter::new("prefix %v suffix");
        let output = fmt.format(&make_record("mid"));
        assert_eq!(output, "prefix mid suffix");
    }

    #[test]
    fn unknown_specifier() {
        let fmt = PatternFormatter::new("%x is unknown");
        let output = fmt.format(&make_record(""));
        assert_eq!(output, "%x is unknown");
    }

    #[test]
    fn empty_args_format() {
        let fmt = PatternFormatter::new("[%l] %v");
        let record = make_record("no args here");
        let output = fmt.format(&record);
        assert_eq!(output, "[Warning] no args here");
    }
}
