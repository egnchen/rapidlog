/// Controls overflow behavior of the per-thread SPSC queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMode {
    /// Grow the queue by chaining new ring buffers when full.
    /// Capacity doubles each growth up to 2 GiB. Never drops messages.
    UnboundedBlocking,
    /// Fixed-size queue that silently drops messages when full.
    BoundedDropping,
}

pub const DEFAULT_START_CAPACITY: usize = 131_072;
pub const MAX_BLOCK_CAPACITY: usize = 2_147_483_648;
