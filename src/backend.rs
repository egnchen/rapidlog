use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::arg;
use crate::formatter::{FormattedRecord, PatternFormatter};
use crate::logger::Logger;
use crate::message::{ArchivedHeader, HEADER_SIZE};
use crate::metadata::Metadata;
use crate::thread_context::ThreadContext;

const DEFAULT_MIN_BATCH_SIZE: usize = 256;

pub struct BackendOptions {
    pub sleep_duration: Duration,
    pub min_batch_size: usize,
    pub pattern_formatter: Option<PatternFormatter>,
}

impl Default for BackendOptions {
    fn default() -> Self {
        Self {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: DEFAULT_MIN_BATCH_SIZE,
            pattern_formatter: None,
        }
    }
}

pub struct BackendHandle {
    thread: Option<JoinHandle<()>>,
    stop_signal: Arc<AtomicBool>,
}

impl BackendHandle {
    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub fn join(&mut self) {
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap();
        }
    }
}

impl Drop for BackendHandle {
    fn drop(&mut self) {
        self.stop();
        self.join();
    }
}

pub struct Backend;

impl Backend {
    pub fn start(options: BackendOptions) -> BackendHandle {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stop_signal);

        let handle = thread::spawn(move || {
            Self::worker_loop(options, stop);
        });

        BackendHandle {
            thread: Some(handle),
            stop_signal,
        }
    }

    fn worker_loop(options: BackendOptions, stop: Arc<AtomicBool>) {
        let formatter = options.pattern_formatter;
        while !stop.load(Ordering::Relaxed) {
            let mut raw_messages = ThreadContext::poll_all_registered_queues();

            if raw_messages.is_empty() {
                thread::sleep(options.sleep_duration);
                continue;
            }

            while raw_messages.len() < options.min_batch_size && !stop.load(Ordering::Relaxed) {
                let more = ThreadContext::poll_all_registered_queues();
                if more.is_empty() {
                    break;
                }
                raw_messages.extend(more);
            }

            let mut decoded: Vec<(ArchivedHeader, Vec<u8>)> = raw_messages
                .into_iter()
                .filter_map(|raw| {
                    let header = ArchivedHeader::decode(&raw)?;
                    Some((header, raw))
                })
                .collect();

            decoded.sort_unstable_by_key(|(m, _)| m.timestamp_ns);

            for (archived, data) in &decoded {
                let metadata: &Metadata = archived.metadata();
                let logger: &Logger = archived.logger();

                if metadata.level.as_usize() < logger.log_level().as_usize() {
                    continue;
                }

                let payload = &data[HEADER_SIZE..];

                let filters = logger.filters.read();
                let filter_pass = filters.iter().all(|f| f.accept(metadata, payload));
                drop(filters);
                if !filter_pass {
                    continue;
                }

                let formatted = Self::format_message(archived, payload, formatter.as_ref());
                for sink in &logger.sinks {
                    sink.write(&formatted);
                }
            }
        }
    }

    fn format_message(
        archived: &ArchivedHeader,
        payload: &[u8],
        formatter: Option<&PatternFormatter>,
    ) -> String {
        let display_ns = crate::timestamp::to_display_nanos(archived.timestamp_ns);
        // SAFETY: metadata_ptr points to a &'static Metadata from the log call site
        // that outlives the backend worker.
        let metadata: &Metadata = archived.metadata();

        let secs = display_ns / 1_000_000_000;
        let nanos = (display_ns % 1_000_000_000) as u32;

        let body = arg::format_body(metadata, payload);

        if let Some(fmtr) = formatter {
            let record = FormattedRecord {
                timestamp_secs: secs,
                timestamp_nanos: nanos,
                level: &metadata.level,
                file: metadata.file,
                line: metadata.line,
                body: &body,
            };
            fmtr.format(&record)
        } else {
            format!(
                "[{}.{:09}] [{:?}] {}:{} {body}",
                secs, nanos, metadata.level, metadata.file, metadata.line,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arg::Encode;
    use crate::filter::LevelFilter;
    use crate::level::LogLevel;
    use crate::logger::Logger;
    use crate::sink::Sink;
    use crate::thread_context::TEST_SERIAL;
    use std::sync::Mutex as StdMutex;

    fn empty_schemas() -> &'static [u8] {
        &[]
    }

    fn schema_one_i32() -> &'static [u8] {
        Box::leak(Box::new([1u8, arg::op_signed_int(3)]))
    }

    fn schema_two_i32() -> &'static [u8] {
        Box::leak(Box::new([
            2u8,
            arg::op_signed_int(3),
            arg::op_signed_int(3),
        ]))
    }

    struct CountingSink {
        count: StdMutex<usize>,
    }

    impl CountingSink {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                count: StdMutex::new(0),
            })
        }
    }

    impl Sink for CountingSink {
        fn write(&self, _formatted: &str) {
            *self.count.lock().unwrap() += 1;
        }

        fn flush(&self) {}
    }

    #[test]
    fn backend_start_and_stop() {
        let _guard = TEST_SERIAL.lock();
        let backend = Backend::start(BackendOptions::default());
        backend.stop();
        let mut backend = backend;
        backend.join();
    }

    #[test]
    fn backend_processes_messages() {
        let _guard = TEST_SERIAL.lock();
        let sink = CountingSink::new();
        let logger = Logger::new("test_backend".to_string(), vec![sink.clone()]);
        logger.set_log_level(LogLevel::TraceL3);

        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "test message {}",
            "test.rs",
            10,
            "test",
            schema_one_i32,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        )));

        // Payload: just encoded i32 (no count/schemas)
        let payload_size = 4;
        let total_size = HEADER_SIZE + payload_size;
        let mut buf = vec![0u8; total_size];

        let header = ArchivedHeader::new(
            1_700_000_000_123_456_789,
            meta,
            Arc::as_ptr(&logger) as *const Logger,
        );
        header.serialize_into(&mut buf[..HEADER_SIZE]);

        42i32.encode_to(&mut buf[HEADER_SIZE..]);

        ThreadContext::init();
        ThreadContext::push(&buf).unwrap();

        let backend = Backend::start(BackendOptions {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: 1,
            ..Default::default()
        });

        thread::sleep(Duration::from_millis(50));
        backend.stop();
        let mut backend = backend;
        backend.join();

        let count = *sink.count.lock().unwrap();
        assert!(count >= 1, "expected at least 1 message, got {count}");
    }

    #[test]
    fn format_message_output() {
        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Warning,
            "fmt: {} {}",
            "src/main.rs",
            42,
            "my_crate",
            schema_two_i32,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        )));

        let logger = Logger::new("test_logger".to_string(), vec![]);
        // Payload: just 2 encoded i32s, no schemas
        let payload_size = 4 + 4;
        let mut buf = vec![0u8; HEADER_SIZE + payload_size];

        let header =
            ArchivedHeader::new(1_700_000_001, meta, Arc::as_ptr(&logger) as *const Logger);
        header.serialize_into(&mut buf[..HEADER_SIZE]);

        123i32.encode_to(&mut buf[HEADER_SIZE..]);
        456i32.encode_to(&mut buf[HEADER_SIZE + 4..]);

        let decoded = ArchivedHeader::decode(&buf).unwrap();
        let payload = &buf[HEADER_SIZE..];
        let formatted = Backend::format_message(&decoded, payload, None);

        assert!(formatted.contains("fmt: 123 456"), "got: {formatted}");
        assert!(formatted.contains("[Warning]"), "got: {formatted}");
        assert!(formatted.contains("src/main.rs:42"), "got: {formatted}");
    }

    #[test]
    fn format_message_with_custom_pattern() {
        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Error,
            "error {}",
            "src/lib.rs",
            100,
            "rapidlog",
            schema_one_i32,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        )));

        let logger = Logger::new("pat_logger".to_string(), vec![]);
        // Payload: just encoded i32, no schemas
        let payload_size = 4;
        let mut buf = vec![0u8; HEADER_SIZE + payload_size];

        let header = ArchivedHeader::new(
            1_700_000_010_000_000_000,
            meta,
            Arc::as_ptr(&logger) as *const Logger,
        );
        header.serialize_into(&mut buf[..HEADER_SIZE]);
        99i32.encode_to(&mut buf[HEADER_SIZE..]);

        let decoded = ArchivedHeader::decode(&buf).unwrap();
        let payload = &buf[HEADER_SIZE..];

        let fmt = PatternFormatter::new("[%l] %F:%L — %v");
        let formatted = Backend::format_message(&decoded, payload, Some(&fmt));

        assert!(
            formatted.starts_with("[Error] src/lib.rs:100"),
            "got: {formatted}"
        );
        assert!(formatted.contains("error 99"), "got: {formatted}");
    }

    #[test]
    fn messages_sorted_by_timestamp() {
        let _guard = TEST_SERIAL.lock();
        let sink = CountingSink::new();
        let logger = Logger::new("test_sort".to_string(), vec![sink.clone()]);
        logger.set_log_level(LogLevel::TraceL3);

        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "sorted",
            "test.rs",
            1,
            "test",
            empty_schemas,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        )));

        for ts in [300, 100, 200] {
            let mut buf = vec![0u8; HEADER_SIZE]; // header only, no payload
            let header = ArchivedHeader::new(ts, meta, Arc::as_ptr(&logger) as *const Logger);
            header.serialize_into(&mut buf);
            ThreadContext::push(&buf).unwrap();
        }

        let backend = Backend::start(BackendOptions {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: 1,
            ..Default::default()
        });

        thread::sleep(Duration::from_millis(50));
        backend.stop();
        let mut backend = backend;
        backend.join();

        assert!(*sink.count.lock().unwrap() >= 3);
    }

    #[test]
    fn filter_blocks_messages() {
        let _guard = TEST_SERIAL.lock();
        let sink = CountingSink::new();
        let logger = Logger::new("test_filter".to_string(), vec![sink.clone()]);
        logger.set_log_level(LogLevel::TraceL3);
        logger.add_filter(Arc::new(LevelFilter::new(LogLevel::Error)));

        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "filtered out",
            "test.rs",
            1,
            "test",
            empty_schemas,
            crate::metadata::empty_string_tables_provider,
            crate::metadata::empty_user_formatters_provider,
        )));

        let mut buf = vec![0u8; HEADER_SIZE]; // header only, no payload
        let header = ArchivedHeader::new(100, meta, Arc::as_ptr(&logger) as *const Logger);
        header.serialize_into(&mut buf);

        ThreadContext::init();
        ThreadContext::push(&buf).unwrap();

        let backend = Backend::start(BackendOptions {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: 1,
            ..Default::default()
        });

        thread::sleep(Duration::from_millis(50));
        backend.stop();
        let mut backend = backend;
        backend.join();

        assert_eq!(*sink.count.lock().unwrap(), 0);
    }
}
