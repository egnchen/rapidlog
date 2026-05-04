use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU8;

use parking_lot::RwLock;

use crate::filter::Filter;
use crate::level::LogLevel;
use crate::sink::Sink;

static LOGGER_REGISTRY: LazyLock<RwLock<HashMap<String, Arc<Logger>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub struct Logger {
    pub name: String,
    pub log_level: AtomicU8,
    pub sinks: Vec<Arc<dyn Sink>>,
    pub(crate) filters: RwLock<Vec<Arc<dyn Filter>>>,
}

impl Logger {
    pub fn new(name: String, sinks: Vec<Arc<dyn Sink>>) -> Arc<Self> {
        let mut registry = LOGGER_REGISTRY.write();
        if let Some(existing) = registry.get(&name) {
            return Arc::clone(existing);
        }
        let logger = Arc::new(Self {
            name,
            log_level: AtomicU8::new(LogLevel::Info.as_usize() as u8),
            sinks,
            filters: RwLock::new(Vec::new()),
        });
        registry.insert(logger.name.clone(), Arc::clone(&logger));
        logger
    }

    pub fn get(name: &str) -> Option<Arc<Self>> {
        let registry = LOGGER_REGISTRY.read();
        registry.get(name).cloned()
    }

    pub fn set_log_level(&self, level: LogLevel) {
        self.log_level
            .store(level.as_usize() as u8, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn log_level(&self) -> LogLevel {
        let val = self.log_level.load(std::sync::atomic::Ordering::Relaxed);
        LogLevel::from_usize(val as usize).unwrap_or(LogLevel::Info)
    }

    /// Adds a runtime filter evaluated by the backend before sink dispatch.
    ///
    /// Multiple filters are AND-combined: a message must pass ALL of them
    /// (in addition to the logger's log level check) to be delivered.
    pub fn add_filter(&self, filter: Arc<dyn Filter>) {
        self.filters.write().push(filter);
    }

    /// Removes all runtime filters from this logger.
    pub fn clear_filters(&self) {
        self.filters.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::LevelFilter;
    use crate::sinks::ConsoleSink;

    fn dummy_sink() -> Arc<dyn Sink> {
        Arc::new(ConsoleSink::new())
    }

    #[test]
    fn create_logger() {
        let logger = Logger::new("test".to_string(), vec![dummy_sink()]);
        assert_eq!(logger.name, "test");
        assert_eq!(logger.log_level(), LogLevel::Info);
    }

    #[test]
    fn set_and_get_log_level() {
        let logger = Logger::new("test_level".to_string(), vec![dummy_sink()]);
        logger.set_log_level(LogLevel::Debug);
        assert_eq!(logger.log_level(), LogLevel::Debug);
        logger.set_log_level(LogLevel::Critical);
        assert_eq!(logger.log_level(), LogLevel::Critical);
    }

    #[test]
    fn create_or_get_returns_existing() {
        let sinks1 = vec![dummy_sink()];
        let sinks2 = vec![];

        let a = Logger::new("shared".to_string(), sinks1);
        let b = Logger::new("shared".to_string(), sinks2);

        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        assert!(Logger::get("nonexistent").is_none());
    }

    #[test]
    fn get_existing_returns_logger() {
        let logger = Logger::new("get_test".to_string(), vec![dummy_sink()]);
        let found = Logger::get("get_test");
        assert!(found.is_some());
        assert!(Arc::ptr_eq(&logger, &found.unwrap()));
    }

    #[test]
    fn add_and_clear_filters() {
        let logger = Logger::new("filter_test".to_string(), vec![dummy_sink()]);
        assert_eq!(logger.filters.read().len(), 0);

        logger.add_filter(Arc::new(LevelFilter::new(LogLevel::Warning)));
        assert_eq!(logger.filters.read().len(), 1);

        logger.add_filter(Arc::new(LevelFilter::new(LogLevel::Error)));
        assert_eq!(logger.filters.read().len(), 2);

        logger.clear_filters();
        assert_eq!(logger.filters.read().len(), 0);
    }
}
