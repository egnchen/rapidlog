use std::cell::RefCell;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::queue::{self, PushError, SpscConsumer, SpscProducer};

static CONSUMER_REGISTRY: LazyLock<Mutex<Vec<SpscConsumer>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

thread_local! {
    static THREAD_CONTEXT: RefCell<ThreadContext> = RefCell::new(ThreadContext::new());
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
        THREAD_CONTEXT.with(|_| {});
    }

    pub fn push(data: &[u8]) -> Result<(), PushError> {
        THREAD_CONTEXT.with(|ctx| ctx.borrow_mut().producer.push(data))
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&ThreadContext) -> R,
    {
        THREAD_CONTEXT.with(|ctx| f(&ctx.borrow()))
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

        // Poll from the main thread
        ThreadContext::init();
        let msgs = ThreadContext::poll_all_registered_queues();

        // We should get all 4 messages, but order between threads is not guaranteed
        let mut sorted: Vec<String> = msgs
            .iter()
            .map(|m| String::from_utf8_lossy(m).to_string())
            .collect();
        sorted.sort();
        assert_eq!(sorted, vec!["t1a", "t1b", "t2a", "t2b"]);
    }

    #[test]
    fn abandoned_queue_cleanup() {
        let (_, cons) = queue::create_queue(1024);
        let mut registry = CONSUMER_REGISTRY.lock();
        // Simulate: add a consumer whose producer has been dropped
        registry.push(cons);
        drop(registry);

        // Poll should clean up the abandoned consumer
        let msgs = ThreadContext::poll_all_registered_queues();
        assert!(msgs.is_empty());

        // After cleanup, the registry should not contain the abandoned consumer
        let registry = CONSUMER_REGISTRY.lock();
        // Note: there may be other registrations from previous tests.
        // We just check the abandoned one was cleaned.
        assert!(registry.iter().all(|c| !c.is_abandoned()));
    }
}
