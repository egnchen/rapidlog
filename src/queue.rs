use rtrb::{Consumer as RtrbConsumer, Producer as RtrbProducer, RingBuffer};

pub const DEFAULT_QUEUE_CAPACITY: usize = 131_072;

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
        let len = data.len() as u32;
        let header = len.to_ne_bytes();
        let total = 4 + data.len();

        if self.inner.slots() < total {
            return Err(PushError::Full);
        }

        let chunk = self
            .inner
            .write_chunk_uninit(total)
            .map_err(|_| PushError::Full)?;
        let iter = header.into_iter().chain(data.iter().copied());
        chunk.fill_from_iter(iter);
        Ok(())
    }
}

impl SpscConsumer {
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        let slots = self.inner.slots();
        if slots < 4 {
            return None;
        }

        let chunk = self.inner.read_chunk(slots).ok()?;
        let (first, second) = chunk.as_slices();

        if first.len() < 4 {
            return None;
        }

        let mut len_buf = [0u8; 4];
        len_buf.copy_from_slice(&first[..4]);
        let msg_len = u32::from_ne_bytes(len_buf) as usize;

        let total_msg = 4 + msg_len;
        let available = first.len() + second.len();
        if available < total_msg {
            return None;
        }

        let mut msg = Vec::with_capacity(msg_len);

        let first_payload_end = total_msg.min(first.len());
        msg.extend_from_slice(&first[4..first_payload_end]);

        let remaining = total_msg.saturating_sub(first.len());
        if remaining > 0 {
            msg.extend_from_slice(&second[..remaining]);
        }

        chunk.commit(total_msg);
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
