use rtrb::{Consumer as RtrbConsumer, Producer as RtrbProducer, RingBuffer};

pub const DEFAULT_QUEUE_CAPACITY: usize = 131_072;

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
}

pub struct SpscConsumer {
    inner: RtrbConsumer<u8>,
}

pub fn create_queue(capacity: usize) -> (SpscProducer, SpscConsumer) {
    let (prod, cons) = RingBuffer::new(capacity);
    (SpscProducer { inner: prod }, SpscConsumer { inner: cons })
}

impl SpscProducer {
    pub fn push(&mut self, data: &[u8]) -> Result<(), PushError> {
        let msg_len = data.len();
        let total = HEADER_SIZE + msg_len;
        let padded = align_up(total, CACHE_LINE);

        if self.inner.slots() < padded {
            return Err(PushError::Full);
        }

        let mut chunk = self
            .inner
            .write_chunk_uninit(padded)
            .map_err(|_| PushError::Full)?;

        let (first, second) = chunk.as_mut_slices();
        let first_ptr = first.as_mut_ptr() as *mut u8;

        unsafe {
            std::ptr::write_bytes(first_ptr, 0, first.len());
            if !second.is_empty() {
                std::ptr::write_bytes(second.as_mut_ptr() as *mut u8, 0, second.len());
            }

            let len_bytes = (msg_len as u32).to_ne_bytes();

            if total <= first.len() {
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), first_ptr, 4);
                std::ptr::copy_nonoverlapping(data.as_ptr(), first_ptr.add(HEADER_SIZE), msg_len);
            } else {
                let first_data = first.len().saturating_sub(HEADER_SIZE);
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), first_ptr, 4);
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    first_ptr.add(HEADER_SIZE),
                    first_data,
                );
                let second_ptr = second.as_mut_ptr() as *mut u8;
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_data),
                    second_ptr,
                    msg_len - first_data,
                );
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
        encode: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, PushError> {
        let total = HEADER_SIZE + total_msg;
        let padded = align_up(total, CACHE_LINE);

        if self.inner.slots() < padded {
            return Err(PushError::Full);
        }

        let mut chunk = self
            .inner
            .write_chunk_uninit(padded)
            .map_err(|_| PushError::Full)?;

        let (first, second) = chunk.as_mut_slices();
        let len_bytes = (total_msg as u32).to_ne_bytes();

        if total <= first.len() {
            unsafe {
                std::ptr::write_bytes(first.as_mut_ptr(), 0, first.len());
                if !second.is_empty() {
                    std::ptr::write_bytes(second.as_mut_ptr() as *mut u8, 0, second.len());
                }
                let p = first.as_mut_ptr() as *mut u8;
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), p, 4);
                let payload = std::slice::from_raw_parts_mut(p.add(HEADER_SIZE), total_msg);
                let result = encode(payload);
                chunk.commit(padded);
                Ok(result)
            }
        } else {
            let mut tmp = vec![0u8; total_msg];
            let result = encode(&mut tmp);

            unsafe {
                std::ptr::write_bytes(first.as_mut_ptr(), 0, first.len());
                let p = first.as_mut_ptr() as *mut u8;
                std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), p, 4);
                let first_data = first.len().saturating_sub(HEADER_SIZE);
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), p.add(HEADER_SIZE), first_data);
                let remaining = total_msg.saturating_sub(first_data);
                if remaining > 0 {
                    std::ptr::write_bytes(second.as_mut_ptr(), 0, second.len());
                    let s = second.as_mut_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(tmp.as_ptr().add(first_data), s, remaining);
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
}
