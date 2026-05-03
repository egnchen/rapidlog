use std::sync::Arc;

use crate::logger::Logger;
use crate::sink::Sink;
use crate::thread_context::ThreadContext;

pub struct Frontend;

impl Frontend {
    pub fn create_or_get_logger(name: &str, sinks: Vec<Arc<dyn Sink>>) -> Arc<Logger> {
        Logger::new(name.to_string(), sinks)
    }

    pub fn preallocate() {
        ThreadContext::init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sinks::ConsoleSink;

    #[test]
    fn create_or_get_creates_logger() {
        let logger =
            Frontend::create_or_get_logger("frontend_test", vec![Arc::new(ConsoleSink::new())]);
        assert_eq!(logger.name, "frontend_test");
    }

    #[test]
    fn create_or_get_returns_same() {
        let a = Frontend::create_or_get_logger("shared", vec![]);
        let b = Frontend::create_or_get_logger("shared", vec![]);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn preallocate_does_not_panic() {
        Frontend::preallocate();
    }
}
