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
