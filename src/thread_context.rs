use std::cell::UnsafeCell;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::config::QueueMode;
use crate::queue::{self, PushError, SpscConsumer, SpscProducer};

static CONSUMER_REGISTRY: LazyLock<Mutex<Vec<SpscConsumer>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

// SAFETY: per-thread, single-owner. No other thread accesses this TLS slot.
// Eagerly initialized — ThreadContext::new() runs on first access.
// No lazy-init branch on the hot path; just CTX.with(|c| c.get()).
thread_local! {
    static CTX: UnsafeCell<ThreadContext> = UnsafeCell::new(ThreadContext::new());
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
        set_or_create_ctx_with_mode(QueueMode::BoundedDropping, queue::DEFAULT_QUEUE_CAPACITY);
    }

    pub fn init_with_mode(mode: QueueMode, start_capacity: usize) {
        set_or_create_ctx_with_mode(mode, start_capacity);
    }

    pub fn push(data: &[u8]) -> Result<(), PushError> {
        // SAFETY: get_ctx() returns a valid, non-aliased *mut ThreadContext
        // that is exclusively owned by this thread via TLS.
        unsafe { &mut *get_ctx() }.push_impl(data)
    }

    fn push_impl(&mut self, data: &[u8]) -> Result<(), PushError> {
        if self.mode == QueueMode::BoundedDropping {
            return self.producer.push(data);
        }
        loop {
            match self.producer.push(data) {
                Ok(()) => return Ok(()),
                Err(PushError::Full) => {
                    if let Some(new_cons) = self.producer.grow() {
                        CONSUMER_REGISTRY.lock().push(new_cons);
                    } else {
                        return Err(PushError::Full);
                    }
                }
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
        if self.mode == QueueMode::BoundedDropping {
            return self.producer.push_encoded(total_msg, encode);
        }
        loop {
            match self.producer.push_encoded(total_msg, encode) {
                Ok(r) => return Ok(r),
                Err(PushError::Full) => {
                    if let Some(new_cons) = self.producer.grow() {
                        CONSUMER_REGISTRY.lock().push(new_cons);
                    } else {
                        return Err(PushError::Full);
                    }
                }
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
    CTX.with(|ctx| unsafe {
        std::ptr::drop_in_place(ctx.get());
        std::ptr::write(ctx.get(), ThreadContext::new());
    });
    let _ = ThreadContext::poll_all_registered_queues();
}

fn set_or_create_ctx_with_mode(mode: QueueMode, start_capacity: usize) -> *mut ThreadContext {
    CTX.with(|ctx| {
        let ptr = ctx.get();
        unsafe {
            let current = &*ptr;
            if current.producer.capacity() != start_capacity {
                std::ptr::drop_in_place(ptr);
                std::ptr::write(ptr, ThreadContext::with_mode(mode, start_capacity));
            } else {
                (&mut *ptr).mode = mode;
            }
        }
        ptr
    })
}

#[inline]
fn get_ctx() -> *mut ThreadContext {
    CTX.with(|ctx| ctx.get())
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
        let _ = ThreadContext::poll_all_registered_queues();

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
