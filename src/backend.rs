use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::arg;
use crate::logger::Logger;
use crate::message::{ARCHIVED_HEADER_SIZE, ArchivedLogMessage, LogMessage};
use crate::metadata::Metadata;
use crate::thread_context::ThreadContext;

const DEFAULT_MIN_BATCH_SIZE: usize = 256;

pub struct BackendOptions {
    pub sleep_duration: Duration,
    pub min_batch_size: usize,
}

impl Default for BackendOptions {
    fn default() -> Self {
        Self {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: DEFAULT_MIN_BATCH_SIZE,
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

            let mut decoded: Vec<(ArchivedLogMessage, Vec<u8>)> = raw_messages
                .iter()
                .filter_map(|raw| {
                    let header = LogMessage::decode(raw)?;
                    let args = raw[ARCHIVED_HEADER_SIZE..][..header.args_len as usize].to_vec();
                    Some((header, args))
                })
                .collect();

            decoded.sort_unstable_by_key(|(m, _)| m.timestamp_ns);

            for (archived, args_bytes) in &decoded {
                let formatted = Self::format_message(archived, args_bytes);
                let metadata: &Metadata =
                    unsafe { &*(archived.metadata_ptr as usize as *const Metadata) };
                let logger: &Logger = unsafe { &*(archived.logger_ptr as usize as *const Logger) };

                if (metadata.level as u8) >= logger.log_level().as_usize() as u8 {
                    for sink in &logger.sinks {
                        sink.write(&formatted);
                    }
                }
            }
        }
    }

    fn format_message(archived: &ArchivedLogMessage, args_bytes: &[u8]) -> String {
        let timestamp_ns = archived.timestamp_ns;
        let metadata: &Metadata = unsafe { &*(archived.metadata_ptr as usize as *const Metadata) };

        let secs = timestamp_ns / 1_000_000_000;
        let nanos = (timestamp_ns % 1_000_000_000) as u32;

        let body = if !args_bytes.is_empty() {
            let decoded = arg::decode_args(args_bytes);
            arg::format_with_args(metadata.format_str, &decoded)
        } else {
            metadata.format_str.to_string()
        };

        format!(
            "[{}.{:09}] [{:?}] {}:{} {body}",
            secs, nanos, metadata.level, metadata.file, metadata.line,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arg::LogArg;
    use crate::level::LogLevel;
    use crate::logger::Logger;
    use crate::sink::Sink;
    use std::sync::Mutex as StdMutex;

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
        let backend = Backend::start(BackendOptions::default());
        backend.stop();
        let mut backend = backend;
        backend.join();
    }

    #[test]
    fn backend_processes_messages() {
        let sink = CountingSink::new();
        let logger = Logger::new("test_backend".to_string(), vec![sink.clone()]);
        logger.set_log_level(LogLevel::TraceL3);

        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "test message {}",
            "test.rs",
            10,
            "test",
        )));

        let arg_count: usize = 1;
        let header_size = 1 + arg_count; // count byte + 1 tag
        let payload_size = 8; // 1 int
        let args_len = header_size + payload_size;
        let total_size = ARCHIVED_HEADER_SIZE + args_len;
        let mut buf = vec![0u8; total_size];

        let msg = LogMessage::new(
            1_700_000_000_123_456_789,
            meta,
            Arc::as_ptr(&logger),
            args_len as u16,
        );
        msg.serialize_header_into(&mut buf[..ARCHIVED_HEADER_SIZE]);

        buf[ARCHIVED_HEADER_SIZE] = 1; // arg_count
        buf[ARCHIVED_HEADER_SIZE + 1] = arg::TAG_INT;

        (42i32).log_encode(&mut buf[ARCHIVED_HEADER_SIZE + 2..]);

        ThreadContext::init();
        ThreadContext::push(&buf).unwrap();

        let backend = Backend::start(BackendOptions {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: 1,
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
        )));

        let logger = Logger::new("test_logger".to_string(), vec![]);
        let total_size = ARCHIVED_HEADER_SIZE + 1 + 2 + 8 + 8;
        let mut buf = vec![0u8; total_size];

        let args_len = 1 + 2 + 8 + 8; // count(1) + tags(2) + int(8) + int(8) = 19
        let msg = LogMessage::new(1_700_000_001, meta, Arc::as_ptr(&logger), args_len as u16);
        msg.serialize_header_into(&mut buf[..ARCHIVED_HEADER_SIZE]);

        buf[ARCHIVED_HEADER_SIZE] = 2; // arg_count
        buf[ARCHIVED_HEADER_SIZE + 1] = arg::TAG_INT;
        buf[ARCHIVED_HEADER_SIZE + 2] = arg::TAG_INT;
        (123i32).log_encode(&mut buf[ARCHIVED_HEADER_SIZE + 3..]);
        (456i32).log_encode(&mut buf[ARCHIVED_HEADER_SIZE + 3 + 8..]);

        let archived = LogMessage::decode(&buf).unwrap();
        let args_bytes = &buf[ARCHIVED_HEADER_SIZE..][..archived.args_len as usize];
        let formatted = Backend::format_message(&archived, args_bytes);

        assert!(formatted.contains("fmt: 123 456"), "got: {formatted}");
        assert!(formatted.contains("[Warning]"), "got: {formatted}");
        assert!(formatted.contains("src/main.rs:42"), "got: {formatted}");
    }

    #[test]
    fn messages_sorted_by_timestamp() {
        let sink = CountingSink::new();
        let logger = Logger::new("test_sort".to_string(), vec![sink.clone()]);
        logger.set_log_level(LogLevel::TraceL3);

        let meta = Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "sorted",
            "test.rs",
            1,
            "test",
        )));

        for ts in [300u64, 100, 200] {
            let total_size = ARCHIVED_HEADER_SIZE + 1; // count=0, no args
            let mut buf = vec![0u8; total_size];
            let msg = LogMessage::new(ts, meta, Arc::as_ptr(&logger), 1);
            msg.serialize_header_into(&mut buf[..ARCHIVED_HEADER_SIZE]);
            buf[ARCHIVED_HEADER_SIZE] = 0; // arg_count = 0
            ThreadContext::push(&buf).unwrap();
        }

        let backend = Backend::start(BackendOptions {
            sleep_duration: Duration::from_millis(1),
            min_batch_size: 1,
        });

        thread::sleep(Duration::from_millis(50));
        backend.stop();
        let mut backend = backend;
        backend.join();

        assert!(*sink.count.lock().unwrap() >= 3);
    }
}
