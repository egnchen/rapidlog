use std::cell::{Cell, RefCell};
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::config::QueueMode;
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
    mode: QueueMode,
}

impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadContext {
    pub fn new() -> Self {
        Self::with_mode(QueueMode::BoundedDropping, queue::DEFAULT_QUEUE_CAPACITY)
    }

    pub fn with_mode(mode: QueueMode, start_capacity: usize) -> Self {
        let (prod, cons) = queue::create_queue(start_capacity);
        CONSUMER_REGISTRY.lock().push(cons);
        Self {
            producer: prod,
            mode,
        }
    }

    pub fn init() {
        ensure_ctx_with_mode(QueueMode::BoundedDropping, queue::DEFAULT_QUEUE_CAPACITY);
    }

    pub fn init_with_mode(mode: QueueMode, start_capacity: usize) {
        ensure_ctx_with_mode(mode, start_capacity);
    }

    pub fn push(data: &[u8]) -> Result<(), PushError> {
        // SAFETY: get_ctx() returns a valid, non-aliased *mut ThreadContext
        // that is exclusively owned by this thread via TLS.
        unsafe { &mut *get_ctx() }.push_impl(data)
    }

    fn push_impl(&mut self, data: &[u8]) -> Result<(), PushError> {
        loop {
            match self.producer.push(data) {
                Ok(()) => return Ok(()),
                Err(PushError::Full) if self.mode == QueueMode::UnboundedBlocking => {
                    if let Some(new_cons) = self.producer.grow() {
                        CONSUMER_REGISTRY.lock().push(new_cons);
                    } else {
                        return Err(PushError::Full);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn push_encoded<R>(
        total_msg: usize,
        mut encode: impl FnMut(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        // SAFETY: get_ctx() returns a valid, non-aliased *mut ThreadContext
        // that is exclusively owned by this thread via TLS.
        unsafe { &mut *get_ctx() }.push_encoded_inner(total_msg, &mut encode)
    }

    fn push_encoded_inner<R>(
        &mut self,
        total_msg: usize,
        encode: &mut impl FnMut(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        loop {
            match self.producer.push_encoded(total_msg, encode) {
                Ok(r) => return Ok(r),
                Err(PushError::Full) if self.mode == QueueMode::UnboundedBlocking => {
                    if let Some(new_cons) = self.producer.grow() {
                        CONSUMER_REGISTRY.lock().push(new_cons);
                    } else {
                        return Err(PushError::Full);
                    }
                }
                Err(e) => return Err(e),
            }
        }
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
                registry.remove(i);
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
pub(crate) static TEST_SERIAL: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

#[cfg(test)]
fn reset_ctx() {
    CTX_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr != 0 {
            CTX_HOLDER.with(|holder| {
                *holder.borrow_mut() = None;
            });
        }
        cell.set(0);
    });
}

fn ensure_ctx_with_mode(mode: QueueMode, start_capacity: usize) -> *mut ThreadContext {
    CTX_PTR.with(|cell| {
        let mut ptr = cell.get();
        if ptr == 0 {
            CTX_HOLDER.with(|holder| {
                let ctx = Box::new(ThreadContext::with_mode(mode, start_capacity));
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
    ensure_ctx_with_mode(QueueMode::BoundedDropping, queue::DEFAULT_QUEUE_CAPACITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn push_and_poll_single_thread() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
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
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init();
        let msgs = ThreadContext::poll_all_registered_queues();
        assert!(msgs.is_empty());
    }

    #[test]
    fn push_from_multiple_threads() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
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
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init();
        assert!(!ThreadContext::has_pending_messages());
        ThreadContext::push(b"data").unwrap();
        assert!(ThreadContext::has_pending_messages());
        let _ = ThreadContext::poll_all_registered_queues();
        assert!(!ThreadContext::has_pending_messages());
    }

    #[test]
    fn abandoned_queue_cleanup() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init();

        let (_, cons) = queue::create_queue(1024);
        let mut registry = CONSUMER_REGISTRY.lock();
        registry.push(cons);
        drop(registry);

        let _msgs = ThreadContext::poll_all_registered_queues();

        let registry = CONSUMER_REGISTRY.lock();
        assert!(registry.iter().all(|c| !c.is_abandoned()));
    }

    #[test]
    fn unbounded_push_past_capacity() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::UnboundedBlocking, 64);

        // Write messages larger than initial capacity
        let payload = vec![42u8; 100];
        for i in 0..20 {
            assert!(
                ThreadContext::push(&payload).is_ok(),
                "push {i} should succeed in unbounded mode"
            );
        }

        // Verify all messages can be popped
        let msgs = ThreadContext::poll_all_registered_queues();
        assert_eq!(msgs.len(), 20);
        for msg in &msgs {
            assert_eq!(msg, &payload);
        }
    }

    #[test]
    fn unbounded_multi_block_pop_fifo() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::UnboundedBlocking, 64);

        // Push messages that force growth
        for i in 0u8..5 {
            let payload = vec![i; 100];
            ThreadContext::push(&payload).unwrap();
        }

        let msgs = ThreadContext::poll_all_registered_queues();
        assert_eq!(msgs.len(), 5);
        for (i, msg) in msgs.iter().enumerate() {
            assert_eq!(msg, &vec![i as u8; 100]);
        }
    }

    #[test]
    fn bounded_drops_on_full() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::BoundedDropping, 64);

        // This should fail because 100 bytes > 64 byte buffer
        let payload = vec![0u8; 100];
        assert_eq!(ThreadContext::push(&payload), Err(PushError::Full));
    }

    #[test]
    fn mode_persistence() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::UnboundedBlocking, 64);

        // Push 10 large messages — should succeed via growth
        for i in 0..10 {
            assert!(ThreadContext::push(&[i as u8; 80]).is_ok());
        }
    }

    #[test]
    fn grow_pushes_consumer_to_registry() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::UnboundedBlocking, 64);

        // Initial registry has 1 consumer
        assert_eq!(CONSUMER_REGISTRY.lock().len(), 1);

        // Push enough to trigger growth
        for _ in 0..10 {
            ThreadContext::push(&[0u8; 80]).unwrap();
        }

        // Registry should have more than 1 consumer (old + new from growth)
        assert!(CONSUMER_REGISTRY.lock().len() > 1);
    }

    #[test]
    fn abandoned_cleanup_with_blocks() {
        let _guard = TEST_SERIAL.lock();
        CONSUMER_REGISTRY.lock().clear();
        reset_ctx();
        ThreadContext::init_with_mode(QueueMode::UnboundedBlocking, 64);

        // Force growth
        for _ in 0..10 {
            ThreadContext::push(&[0u8; 80]).unwrap();
        }

        let consumer_count_before = CONSUMER_REGISTRY.lock().len();
        assert!(consumer_count_before > 1);

        // Drain all messages
        let _msgs = ThreadContext::poll_all_registered_queues();

        // After draining, old abandoned consumers are removed.
        // The active consumer (last one) is NOT abandoned, so it stays.
        let registry = CONSUMER_REGISTRY.lock();
        assert!(!registry.is_empty(), "active consumer must remain");
    }
}
