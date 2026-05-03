use crate::queue::{PushError, SpscQueue};
use std::cell::RefCell;

thread_local! {
    static THREAD_CONTEXT: RefCell<ThreadContext> = RefCell::new(ThreadContext::new());
}

pub struct ThreadContext {
    queue: SpscQueue,
}

impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadContext {
    pub fn new() -> Self {
        Self {
            queue: SpscQueue::new(131_072),
        }
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&ThreadContext) -> R,
    {
        THREAD_CONTEXT.with(|ctx| f(&ctx.borrow()))
    }

    pub fn push(&self, data: &[u8]) -> Result<(), PushError> {
        THREAD_CONTEXT.with(|ctx| ctx.borrow_mut().queue.push(data))
    }

    pub fn pop_all(&self) -> Vec<u8> {
        THREAD_CONTEXT.with(|ctx| ctx.borrow_mut().queue.pop_all())
    }

    pub fn is_empty(&self) -> bool {
        THREAD_CONTEXT.with(|ctx| ctx.borrow().queue.is_empty())
    }
}
