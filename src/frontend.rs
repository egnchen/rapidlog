use crate::logger::Logger;
use crate::sink::Sink;
use std::sync::Arc;

pub struct Frontend;

impl Frontend {
    pub fn create_or_get_logger(_name: &str, sinks: Vec<Arc<dyn Sink>>) -> Arc<Logger> {
        Logger::new(_name.to_string(), sinks)
    }

    pub fn preallocate() {}
}
