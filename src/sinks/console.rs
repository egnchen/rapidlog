use std::io::{Write, stdout};

use anstream::{AutoStream, ColorChoice};

use crate::sink::Sink;

pub struct ConsoleSink {
    stream: parking_lot::Mutex<AutoStream<std::io::Stdout>>,
}

impl Default for ConsoleSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleSink {
    pub fn new() -> Self {
        Self {
            stream: parking_lot::Mutex::new(AutoStream::new(stdout(), ColorChoice::Auto)),
        }
    }
}

impl Sink for ConsoleSink {
    fn write(&self, formatted: &str) {
        let mut stream = self.stream.lock();
        let _ = writeln!(stream, "{formatted}");
    }

    fn flush(&self) {
        let mut stream = self.stream.lock();
        let _ = stream.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_sink_write_and_flush() {
        let sink = ConsoleSink::new();
        sink.write("test message");
        sink.flush();
    }

    #[test]
    fn console_sink_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ConsoleSink>();
        assert_sync::<ConsoleSink>();
    }
}
