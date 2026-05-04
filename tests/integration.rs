use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use rapidlog::{Backend, BackendOptions, Frontend, LevelFilter, LogLevel, Sink};

struct CountingSink {
    count: AtomicUsize,
    messages: Mutex<Vec<String>>,
}

impl CountingSink {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            count: AtomicUsize::new(0),
            messages: Mutex::new(Vec::new()),
        })
    }
}

impl Sink for CountingSink {
    fn write(&self, formatted: &str) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.messages.lock().unwrap().push(formatted.to_string());
    }

    fn flush(&self) {}
}

#[test]
fn integration_single_thread_logging() {
    let sink = CountingSink::new();
    let logger = Frontend::create_or_get_logger("integration_single", vec![sink.clone()]);
    logger.set_log_level(LogLevel::TraceL3);

    let backend = Backend::start(BackendOptions {
        sleep_duration: Duration::from_millis(1),
        min_batch_size: 1,
    });

    Frontend::preallocate();

    let num_messages = 100;
    for i in 0..num_messages {
        rapidlog::log_info!(logger, "message {}", i);
    }

    // Wait for backend to process
    let start = std::time::Instant::now();
    loop {
        if sink.count.load(Ordering::Relaxed) >= num_messages {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!(
                "timeout: only {}/{} messages processed",
                sink.count.load(Ordering::Relaxed),
                num_messages
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    backend.stop();
    let mut backend = backend;
    backend.join();

    assert_eq!(sink.count.load(Ordering::Relaxed), num_messages);

    let msgs = sink.messages.lock().unwrap();
    assert_eq!(msgs.len(), num_messages);
    for i in 0..num_messages {
        let expected = format!("message {i}");
        assert!(
            msgs.iter().any(|m| m.contains(&expected)),
            "missing message: {expected}"
        );
    }
}

#[test]
fn integration_multi_thread_logging() {
    let sink = CountingSink::new();
    let logger = Frontend::create_or_get_logger("integration_multi", vec![sink.clone()]);
    logger.set_log_level(LogLevel::TraceL3);

    let backend = Backend::start(BackendOptions {
        sleep_duration: Duration::from_millis(1),
        min_batch_size: 1,
    });

    let num_threads = 4;
    let msgs_per_thread = 50;
    let total_messages = num_threads * msgs_per_thread;
    let mut handles = vec![];

    for t in 0..num_threads {
        let logger = logger.clone();
        let handle = thread::spawn(move || {
            Frontend::preallocate();
            for i in 0..msgs_per_thread {
                rapidlog::log_info!(logger, "thread_{}_msg_{}", t, i);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Wait for backend to process
    let start = std::time::Instant::now();
    loop {
        if sink.count.load(Ordering::Relaxed) >= total_messages {
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            panic!(
                "timeout: only {}/{} messages processed",
                sink.count.load(Ordering::Relaxed),
                total_messages
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    backend.stop();
    let mut backend = backend;
    backend.join();

    assert_eq!(sink.count.load(Ordering::Relaxed), total_messages);

    let msgs = sink.messages.lock().unwrap();
    assert_eq!(msgs.len(), total_messages);

    for t in 0..num_threads {
        for i in 0..msgs_per_thread {
            let expected = format!("thread_{t}_msg_{i}");
            assert!(
                msgs.iter().any(|m| m.contains(&expected)),
                "missing message: {expected}"
            );
        }
    }
}

#[test]
fn integration_level_filtering() {
    let sink = CountingSink::new();
    let logger = Frontend::create_or_get_logger("integration_filter", vec![sink.clone()]);
    logger.set_log_level(LogLevel::Warning);

    let backend = Backend::start(BackendOptions {
        sleep_duration: Duration::from_millis(1),
        min_batch_size: 1,
    });

    Frontend::preallocate();

    // These should be filtered out
    rapidlog::log_debug!(logger, "debug filtered");
    rapidlog::log_info!(logger, "info filtered");

    // These should pass
    rapidlog::log_warning!(logger, "warning passes {}", 1);
    rapidlog::log_error!(logger, "error passes {}", 2);
    rapidlog::log_critical!(logger, "critical passes {}", 3);

    let expected_count = 3;

    let start = std::time::Instant::now();
    loop {
        if sink.count.load(Ordering::Relaxed) >= expected_count {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!(
                "timeout: only {}/{} messages processed",
                sink.count.load(Ordering::Relaxed),
                expected_count
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    backend.stop();
    let mut backend = backend;
    backend.join();

    assert_eq!(sink.count.load(Ordering::Relaxed), expected_count);

    let msgs = sink.messages.lock().unwrap();
    assert_eq!(msgs.len(), expected_count);
    assert!(msgs.iter().any(|m| m.contains("warning passes")));
    assert!(msgs.iter().any(|m| m.contains("error passes")));
    assert!(msgs.iter().any(|m| m.contains("critical passes")));
    assert!(!msgs.iter().any(|m| m.contains("filtered")));
}

#[test]
fn integration_filter_trait_blocking() {
    let sink = CountingSink::new();
    let logger = Frontend::create_or_get_logger("integration_filter_trait", vec![sink.clone()]);
    logger.set_log_level(LogLevel::TraceL3);
    logger.add_filter(std::sync::Arc::new(LevelFilter::new(LogLevel::Warning)));

    let backend = Backend::start(BackendOptions {
        sleep_duration: Duration::from_millis(1),
        min_batch_size: 1,
    });

    Frontend::preallocate();

    rapidlog::log_debug!(logger, "debug filtered");
    rapidlog::log_info!(logger, "info filtered");
    rapidlog::log_warning!(logger, "warning passes {}", 1);
    rapidlog::log_error!(logger, "error passes {}", 2);

    let expected_count = 2;

    let start = std::time::Instant::now();
    loop {
        if sink.count.load(Ordering::Relaxed) >= expected_count {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!(
                "timeout: only {}/{} messages processed",
                sink.count.load(Ordering::Relaxed),
                expected_count
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    backend.stop();
    let mut backend = backend;
    backend.join();

    assert_eq!(sink.count.load(Ordering::Relaxed), expected_count);

    let msgs = sink.messages.lock().unwrap();
    assert_eq!(msgs.len(), expected_count);
    assert!(msgs.iter().any(|m| m.contains("warning passes")));
    assert!(msgs.iter().any(|m| m.contains("error passes")));
    assert!(!msgs.iter().any(|m| m.contains("filtered")));
}

#[test]
fn integration_timestamps_are_present() {
    let sink = CountingSink::new();
    let logger = Frontend::create_or_get_logger("integration_ts", vec![sink.clone()]);
    logger.set_log_level(LogLevel::TraceL3);

    let backend = Backend::start(BackendOptions {
        sleep_duration: Duration::from_millis(1),
        min_batch_size: 1,
    });

    Frontend::preallocate();

    let num_messages = 10;
    for i in 0..num_messages {
        rapidlog::log_info!(logger, "ts_msg {}", i);
        std::thread::sleep(Duration::from_millis(2));
    }

    let start = std::time::Instant::now();
    loop {
        if sink.count.load(Ordering::Relaxed) >= num_messages {
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            panic!(
                "timeout: only {}/{} messages processed",
                sink.count.load(Ordering::Relaxed),
                num_messages
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    backend.stop();
    let mut backend = backend;
    backend.join();

    assert_eq!(sink.count.load(Ordering::Relaxed), num_messages);

    let msgs = sink.messages.lock().unwrap();
    assert_eq!(msgs.len(), num_messages);

    for i in 0..num_messages {
        let expected = format!("ts_msg {i}");
        assert!(
            msgs.iter().any(|m| m.contains(&expected)),
            "missing: {expected}"
        );
    }

    // Verify every message has a valid timestamp prefix: [digits.digits]
    for msg in msgs.iter() {
        assert!(
            msg.starts_with('['),
            "message missing timestamp bracket: {msg}"
        );
        let rest = &msg[1..];
        let dot = rest.find('.').expect("no dot in timestamp");
        let space = rest.find(']').expect("no closing bracket in timestamp");
        let secs: u64 = rest[..dot].parse().expect("invalid seconds in timestamp");
        let nanos: u32 = rest[dot + 1..space]
            .parse()
            .expect("invalid nanos in timestamp");
        assert!(nanos < 1_000_000_000);
        assert!(secs > 1_700_000_000, "timestamp too old: {secs}");
    }
}
