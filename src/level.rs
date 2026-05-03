#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    TraceL3 = 0,
    TraceL2 = 1,
    TraceL1 = 2,
    Debug = 3,
    Info = 4,
    Warning = 5,
    Error = 6,
    Critical = 7,
}

impl LogLevel {
    pub fn as_usize(self) -> usize {
        self as usize
    }

    pub fn from_usize(val: usize) -> Option<Self> {
        match val {
            0 => Some(Self::TraceL3),
            1 => Some(Self::TraceL2),
            2 => Some(Self::TraceL1),
            3 => Some(Self::Debug),
            4 => Some(Self::Info),
            5 => Some(Self::Warning),
            6 => Some(Self::Error),
            7 => Some(Self::Critical),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_levels() {
        let levels = [
            LogLevel::TraceL3,
            LogLevel::TraceL2,
            LogLevel::TraceL1,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
            LogLevel::Critical,
        ];
        for orig in levels {
            let val = orig.as_usize();
            let restored = LogLevel::from_usize(val);
            assert_eq!(restored, Some(orig), "round-trip failed for {:?}", orig);
        }
    }

    #[test]
    fn from_usize_out_of_range() {
        assert_eq!(LogLevel::from_usize(8), None);
        assert_eq!(LogLevel::from_usize(usize::MAX), None);
    }

    #[test]
    fn as_usize_values() {
        assert_eq!(LogLevel::TraceL3.as_usize(), 0);
        assert_eq!(LogLevel::TraceL2.as_usize(), 1);
        assert_eq!(LogLevel::TraceL1.as_usize(), 2);
        assert_eq!(LogLevel::Debug.as_usize(), 3);
        assert_eq!(LogLevel::Info.as_usize(), 4);
        assert_eq!(LogLevel::Warning.as_usize(), 5);
        assert_eq!(LogLevel::Error.as_usize(), 6);
        assert_eq!(LogLevel::Critical.as_usize(), 7);
    }

    #[test]
    fn ordering() {
        assert!(LogLevel::TraceL3 < LogLevel::Info);
        assert!(LogLevel::Debug < LogLevel::Warning);
        assert_eq!(LogLevel::Critical, LogLevel::Critical);
    }
}
