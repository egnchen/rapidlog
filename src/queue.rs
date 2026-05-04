use crate::config::MAX_BLOCK_CAPACITY;
use rtrb::{Consumer as RtrbConsumer, Producer as RtrbProducer, RingBuffer};

pub use crate::config::DEFAULT_START_CAPACITY as DEFAULT_QUEUE_CAPACITY;

pub const HEADER_SIZE: usize = 8;

pub const CACHE_LINE: usize = 64;

const fn align_up(n: usize, align: usize) -> usize {
    (n + align - 1) & !(align - 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushError {
    Full,
}

pub struct SpscProducer {
    inner: RtrbProducer<u8>,
    capacity: usize,
}

pub struct SpscConsumer {
    inner: RtrbConsumer<u8>,
}

pub fn create_queue(capacity: usize) -> (SpscProducer, SpscConsumer) {
    let (prod, cons) = RingBuffer::new(capacity);
    (
        SpscProducer {
            inner: prod,
            capacity,
        },
        SpscConsumer { inner: cons },
    )
}

impl SpscProducer {
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    // Creates a new ring buffer at doubled capacity and returns its consumer half.
    // The old producer is dropped, marking the old consumer as abandoned. The caller
    // must push the new consumer into CONSUMER_REGISTRY so the backend drains both
    // the old block (abandoned, via registry cleanup) and the new block.
    pub fn grow(&mut self) -> Option<SpscConsumer> {
        let new_cap = (self.capacity * 2).min(MAX_BLOCK_CAPACITY);
        if new_cap <= self.capacity {
            return None;
        }
        let (prod, cons) = RingBuffer::new(new_cap);
        self.inner = prod;
        self.capacity = new_cap;
        Some(SpscConsumer { inner: cons })
    }

    pub fn push(&mut self, data: &[u8]) -> Result<(), PushError> {
        let msg_len = data.len();
        let total = HEADER_SIZE + msg_len;
        let padded = align_up(total, CACHE_LINE);

        let mut chunk = self
            .inner
            .write_chunk_uninit(padded)
            .map_err(|_| PushError::Full)?;

        let (first, second) = chunk.as_mut_slices();
        let first_ptr = first.as_mut_ptr() as *mut u8;
        let header = msg_len as u32;

        // SAFETY: chunk was obtained from write_chunk_uninit, guaranteeing
        // at least padded bytes of uninitialized memory. We write the 8-byte
        // header + payload + zero-fill padding to exactly padded bytes.
        unsafe {
            if total <= first.len() {
                (first_ptr as *mut u32).write_unaligned(header);
                std::ptr::copy_nonoverlapping(data.as_ptr(), first_ptr.add(HEADER_SIZE), msg_len);
                let pad = first.len() - total;
                if pad > 0 {
                    std::ptr::write_bytes(first_ptr.add(total), 0, pad);
                }
                if !second.is_empty() {
                    std::ptr::write_bytes(second.as_mut_ptr() as *mut u8, 0, second.len());
                }
            } else {
                let first_data = first.len().saturating_sub(HEADER_SIZE);
                (first_ptr as *mut u32).write_unaligned(header);
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    first_ptr.add(HEADER_SIZE),
                    first_data,
                );
                let remaining = msg_len - first_data;
                let s = second.as_mut_ptr() as *mut u8;
                std::ptr::copy_nonoverlapping(data.as_ptr().add(first_data), s, remaining);
                let pad = second.len().saturating_sub(remaining);
                if pad > 0 {
                    std::ptr::write_bytes(s.add(remaining), 0, pad);
                }
            }
        }

        unsafe {
            chunk.commit(padded);
        }
        Ok(())
    }

    pub fn push_encoded<R>(
        &mut self,
        total_msg: usize,
        encode: &mut impl FnMut(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        let total = HEADER_SIZE + total_msg;
        let padded = align_up(total, CACHE_LINE);

        let mut chunk = self
            .inner
            .write_chunk_uninit(padded)
            .map_err(|_| PushError::Full)?;

        let (first, second) = chunk.as_mut_slices();
        let header = total_msg as u32;

        if total <= first.len() {
            // SAFETY: write_chunk_uninit guarantees at least padded bytes.
            // We write header + encode payload + zero-fill padding.
            unsafe {
                let p = first.as_mut_ptr() as *mut u8;
                (p as *mut u32).write_unaligned(header);
                let payload = std::slice::from_raw_parts_mut(p.add(HEADER_SIZE), total_msg);
                let result = encode(payload);
                let pad = first.len() - total;
                if pad > 0 {
                    std::ptr::write_bytes(p.add(total), 0, pad);
                }
                if !second.is_empty() {
                    std::ptr::write_bytes(second.as_mut_ptr() as *mut u8, 0, second.len());
                }
                chunk.commit(padded);
                Ok(result)
            }
        } else {
            let mut tmp = vec![0u8; total_msg];
            let result = encode(&mut tmp);

            // SAFETY: chunk bounds checked; tmp holds exactly total_msg bytes.
            unsafe {
                let p = first.as_mut_ptr() as *mut u8;
                (p as *mut u32).write_unaligned(header);
                let first_data = first.len().saturating_sub(HEADER_SIZE);
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), p.add(HEADER_SIZE), first_data);
                let remaining = total_msg.saturating_sub(first_data);
                if remaining > 0 {
                    let s = second.as_mut_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(tmp.as_ptr().add(first_data), s, remaining);
                    let pad = second.len().saturating_sub(remaining);
                    if pad > 0 {
                        std::ptr::write_bytes(s.add(remaining), 0, pad);
                    }
                }
                chunk.commit(padded);
            }
            Ok(result)
        }
    }
}

impl SpscConsumer {
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        let slots = self.inner.slots();
        if slots < HEADER_SIZE {
            return None;
        }

        let chunk = self.inner.read_chunk(slots).ok()?;
        let (first, second) = chunk.as_slices();

        if first.len() < HEADER_SIZE {
            return None;
        }

        let mut len_buf = [0u8; 4];
        len_buf.copy_from_slice(&first[..4]);
        let msg_len = u32::from_ne_bytes(len_buf) as usize;

        let padded = align_up(HEADER_SIZE + msg_len, CACHE_LINE);
        let available = first.len() + second.len();
        if available < padded {
            return None;
        }

        let mut msg = Vec::with_capacity(msg_len);

        let first_payload = (first.len() - HEADER_SIZE).min(msg_len);
        if first_payload > 0 {
            msg.extend_from_slice(&first[HEADER_SIZE..HEADER_SIZE + first_payload]);
        }

        let remaining = msg_len.saturating_sub(first_payload);
        if remaining > 0 {
            msg.extend_from_slice(&second[..remaining.min(second.len())]);
        }

        chunk.commit(padded);
        Some(msg)
    }

    pub fn pop_all(&mut self) -> Vec<Vec<u8>> {
        let mut msgs = Vec::new();
        while let Some(msg) = self.pop() {
            msgs.push(msg);
        }
        msgs
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn is_abandoned(&self) -> bool {
        self.inner.is_abandoned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop_single_message() {
        let (mut prod, mut cons) = create_queue(1024);
        let payload = b"hello world";
        assert!(prod.push(payload).is_ok());
        assert!(!cons.is_empty());
        let msg = cons.pop().unwrap();
        assert_eq!(msg, payload);
        assert!(cons.is_empty());
    }

    #[test]
    fn push_and_pop_multiple_messages() {
        let (mut prod, mut cons) = create_queue(1024);
        for i in 0..10 {
            let payload = format!("msg{}", i);
            assert!(prod.push(payload.as_bytes()).is_ok());
        }
        let msgs = cons.pop_all();
        assert_eq!(msgs.len(), 10);
        for (i, msg) in msgs.iter().enumerate() {
            assert_eq!(msg, format!("msg{}", i).as_bytes());
        }
    }

    #[test]
    fn push_full_queue() {
        let (mut prod, cons) = create_queue(64);
        let payload = vec![0u8; 100];
        assert_eq!(prod.push(&payload), Err(PushError::Full));
        assert!(cons.is_empty());
    }

    #[test]
    fn pop_empty_queue() {
        let (_prod, mut cons) = create_queue(1024);
        assert!(cons.pop().is_none());
        assert!(cons.is_empty());
    }

    #[test]
    fn pop_all_on_empty_queue() {
        let (_prod, mut cons) = create_queue(1024);
        let msgs = cons.pop_all();
        assert!(msgs.is_empty());
    }

    #[test]
    fn mixed_small_and_large_messages() {
        let (mut prod, mut cons) = create_queue(8192);
        let small = b"x";
        let large = vec![42u8; 1000];

        assert!(prod.push(small).is_ok());
        assert!(prod.push(&large).is_ok());
        assert!(prod.push(small).is_ok());

        let msgs = cons.pop_all();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0], small);
        assert_eq!(msgs[1], large);
        assert_eq!(msgs[2], small);
    }

    #[test]
    fn empty_message_roundtrip() {
        let (mut prod, mut cons) = create_queue(1024);
        let empty: &[u8] = &[];
        assert!(prod.push(empty).is_ok());
        let msg = cons.pop().unwrap();
        assert!(msg.is_empty());
    }

    #[test]
    fn abandoned_detection() {
        let (prod, cons) = create_queue(1024);
        assert!(!cons.is_abandoned());
        drop(prod);
        assert!(cons.is_abandoned());
    }

    #[test]
    fn capacity_returns_correct_value() {
        let (prod, _cons) = create_queue(256);
        assert_eq!(prod.capacity(), 256);
        let (prod, _cons) = create_queue(1024);
        assert_eq!(prod.capacity(), 1024);
    }

    #[test]
    fn grow_doubles_capacity() {
        let (mut prod, _cons) = create_queue(256);
        assert_eq!(prod.capacity(), 256);
        let new_cons = prod.grow().unwrap();
        assert_eq!(prod.capacity(), 512);
        assert!(!new_cons.is_abandoned());
    }

    #[test]
    fn grow_respects_max_and_returns_none() {
        // Create producer at max block capacity
        let max = MAX_BLOCK_CAPACITY;
        let (mut prod, _cons) = create_queue(max);
        assert!(prod.grow().is_none());
        assert_eq!(prod.capacity(), max);
    }
}
