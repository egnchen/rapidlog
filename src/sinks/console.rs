use crate::sink::Sink;

pub struct ConsoleSink;

impl Default for ConsoleSink {
    fn default() -> Self {
        Self
    }
}

impl ConsoleSink {
    pub fn new() -> Self {
        Self
    }
}

impl Sink for ConsoleSink {
    fn write(&self, formatted: &str) {
        println!("{formatted}");
    }

    fn flush(&self) {}
}
