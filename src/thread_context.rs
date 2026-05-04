use std::cell::{Cell, RefCell};
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::queue::{self, PushError, SpscConsumer, SpscProducer};

static CONSUMER_REGISTRY: LazyLock<Mutex<Vec<SpscConsumer>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

// SAFETY: per-thread, single-owner. No other thread accesses these TLS slots.
// CTX_PTR: const { Cell::new(0) } triggers local-exec TLS model — no lazy-init branch,
// compiler emits `mov reg, fs:[offset]` on x86_64 Linux.
// CTX_HOLDER: owns the Box<ThreadContext> so TLS destructor drops it (frees producer,
// marks consumer abandoned for cleanup).
thread_local! {
    static CTX_PTR: Cell<usize> = const { Cell::new(0) };
    static CTX_HOLDER: RefCell<Option<Box<ThreadContext>>> = const { RefCell::new(None) };
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
        ensure_ctx();
    }

    pub fn push(data: &[u8]) -> Result<(), PushError> {
        // SAFETY: get_ctx() returns a valid, non-aliased *mut ThreadContext
        // that is exclusively owned by this thread via TLS.
        unsafe { &mut *get_ctx() }.producer.push(data)
    }

    pub fn push_encoded<R>(
        total_msg: usize,
        encode: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        // SAFETY: get_ctx() returns a valid, non-aliased *mut ThreadContext
        // that is exclusively owned by this thread via TLS.
        unsafe { &mut *get_ctx() }
            .producer
            .push_encoded(total_msg, encode)
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&ThreadContext) -> R,
    {
        // SAFETY: get_ctx() returns a valid, properly-aligned pointer.
        f(unsafe { &*get_ctx() })
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
pub(crate) static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn ensure_ctx() -> *mut ThreadContext {
    CTX_PTR.with(|cell| {
        let mut ptr = cell.get();
        if ptr == 0 {
            CTX_HOLDER.with(|holder| {
                let ctx = Box::new(ThreadContext::new());
                ptr = Box::into_raw(ctx) as usize;
                cell.set(ptr);
                // SAFETY: ptr came from Box::into_raw on the same allocation
                // one line above. Re-boxing for TLS destructor cleanup.
                let cleanup = unsafe { Box::from_raw(ptr as *mut ThreadContext) };
                *holder.borrow_mut() = Some(cleanup);
            });
        }
        ptr as *mut ThreadContext
    })
}

#[inline]
fn get_ctx() -> *mut ThreadContext {
    ensure_ctx()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn push_and_poll_single_thread() {
        let _guard = TEST_SERIAL.lock().unwrap();
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
        let _guard = TEST_SERIAL.lock().unwrap();
        ThreadContext::init();
        let msgs = ThreadContext::poll_all_registered_queues();
        assert!(msgs.is_empty());
    }

    #[test]
    fn push_from_multiple_threads() {
        let _guard = TEST_SERIAL.lock().unwrap();
        ThreadContext::init();

        let t1 = thread::spawn(|| {
            ThreadContext::push(b"t1a").unwrap();
            ThreadContext::push(b"t1b").unwrap();
        });

        let t2 = thread::spawn(|| {
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
    fn has_pending_messages_detects_data() {
        let _guard = TEST_SERIAL.lock().unwrap();
        ThreadContext::init();
        assert!(!ThreadContext::has_pending_messages());
        ThreadContext::push(b"data").unwrap();
        assert!(ThreadContext::has_pending_messages());
        let _ = ThreadContext::poll_all_registered_queues();
        assert!(!ThreadContext::has_pending_messages());
    }

    #[test]
    fn abandoned_queue_cleanup() {
        let _guard = TEST_SERIAL.lock().unwrap();
        ThreadContext::init();

        let (_, cons) = queue::create_queue(1024);
        let mut registry = CONSUMER_REGISTRY.lock();
        registry.push(cons);
        drop(registry);

        let _msgs = ThreadContext::poll_all_registered_queues();

        let registry = CONSUMER_REGISTRY.lock();
        assert!(registry.iter().all(|c| !c.is_abandoned()));
    }
}
