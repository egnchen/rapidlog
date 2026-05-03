use crate::level::LogLevel;
use crate::sink::Sink;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

pub struct Logger {
    pub name: String,
    pub log_level: AtomicU8,
    pub sinks: Vec<Arc<dyn Sink>>,
}

impl Logger {
    pub fn new(name: String, sinks: Vec<Arc<dyn Sink>>) -> Arc<Self> {
        Arc::new(Self {
            name,
            log_level: AtomicU8::new(LogLevel::Info.as_usize() as u8),
            sinks,
        })
    }

    pub fn set_log_level(&self, level: LogLevel) {
        self.log_level
            .store(level.as_usize() as u8, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn log_level(&self) -> LogLevel {
        let val = self.log_level.load(std::sync::atomic::Ordering::Relaxed);
        LogLevel::from_usize(val as usize).unwrap_or(LogLevel::Info)
    }
}
