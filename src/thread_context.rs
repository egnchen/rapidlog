use std::cell::UnsafeCell;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::queue::{self, PushError, SpscConsumer, SpscProducer};

static CONSUMER_REGISTRY: LazyLock<Mutex<Vec<SpscConsumer>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

// SAFETY: per-thread, single-owner. No other thread can access THREAD_CTX.
// The closure in push/push_encoded is called synchronously, not re-entrant.
thread_local! {
    static THREAD_CTX: UnsafeCell<ThreadContext> = UnsafeCell::new(ThreadContext::new());
}

pub struct ThreadContext {
    producer: SpscProducer,
}

impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadContext {
    pub fn new() -> Self {
        let (prod, cons) = queue::create_queue(queue::DEFAULT_QUEUE_CAPACITY);
        CONSUMER_REGISTRY.lock().push(cons);
        Self { producer: prod }
    }

    pub fn init() {
        THREAD_CTX.with(|_| {});
    }

    pub fn push(data: &[u8]) -> Result<(), PushError> {
        THREAD_CTX.with(|ctx| unsafe { &mut *ctx.get() }.producer.push(data))
    }

    pub fn push_encoded<R>(
        total_msg: usize,
        encode: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        THREAD_CTX.with(|ctx| {
            unsafe { &mut *ctx.get() }
                .producer
                .push_encoded(total_msg, encode)
        })
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&ThreadContext) -> R,
    {
        THREAD_CTX.with(|ctx| f(unsafe { &*ctx.get() }))
    }

    pub fn poll_all_registered_queues() -> Vec<Vec<u8>> {
        let mut registry = CONSUMER_REGISTRY.lock();
        let mut all_msgs = Vec::new();
        let mut i = 0;
        while i < registry.len() {
            let msgs = registry[i].pop_all();
            all_msgs.extend(msgs);
            if registry[i].is_abandoned() && registry[i].is_empty() {
                registry.swap_remove(i);
            } else {
                i += 1;
            }
        }
        all_msgs
    }

    pub fn has_pending_messages() -> bool {
        let registry = CONSUMER_REGISTRY.lock();
        registry.iter().any(|c| !c.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn push_and_poll_single_thread() {
        ThreadContext::init();
        ThreadContext::push(b"hello").unwrap();
        ThreadContext::push(b"world").unwrap();

        let msgs = ThreadContext::poll_all_registered_queues();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0], b"hello");
        assert_eq!(msgs[1], b"world");
    }

    #[test]
    fn poll_empty_returns_empty() {
        ThreadContext::init();
        let msgs = ThreadContext::poll_all_registered_queues();
        assert!(msgs.is_empty());
    }

    #[test]
    fn push_from_multiple_threads() {
        // Drain stale state
        ThreadContext::init();
        let _ = ThreadContext::poll_all_registered_queues();

        let t1 = thread::spawn(|| {
            ThreadContext::init();
            ThreadContext::push(b"t1a").unwrap();
            ThreadContext::push(b"t1b").unwrap();
        });

        let t2 = thread::spawn(|| {
            ThreadContext::init();
            ThreadContext::push(b"t2a").unwrap();
            ThreadContext::push(b"t2b").unwrap();
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let msgs = ThreadContext::poll_all_registered_queues();

        let mut sorted: Vec<String> = msgs
            .iter()
            .map(|m| String::from_utf8_lossy(m).to_string())
            .collect();
        sorted.sort();
        assert_eq!(sorted, vec!["t1a", "t1b", "t2a", "t2b"]);
    }

    #[test]
    fn abandoned_queue_cleanup() {
        // Drain stale state from previous tests sharing CONSUMER_REGISTRY
        ThreadContext::init();
        let _ = ThreadContext::poll_all_registered_queues();

        let (_, cons) = queue::create_queue(1024);
        let mut registry = CONSUMER_REGISTRY.lock();
        registry.push(cons);
        drop(registry);

        let msgs = ThreadContext::poll_all_registered_queues();
        assert!(msgs.is_empty());

        let registry = CONSUMER_REGISTRY.lock();
        assert!(registry.iter().all(|c| !c.is_abandoned()));
    }
}
