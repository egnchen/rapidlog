use crate::sink::Sink;

/// A no-op sink that discards all messages.
///
/// Useful for benchmarks and tests where output is not needed.
pub struct NullSink;

impl Default for NullSink {
    fn default() -> Self {
        Self
    }
}

impl Sink for NullSink {
    fn write(&self, _formatted: &str) {}

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_flush_no_panic() {
        let sink = NullSink;
        sink.write("anything");
        sink.write("can go here");
        sink.flush();
    }

    #[test]
    fn null_sink_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<NullSink>();
        assert_sync::<NullSink>();
    }
}
