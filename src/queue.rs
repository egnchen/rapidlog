use std::collections::VecDeque;

pub struct SpscQueue {
    buffer: VecDeque<u8>,
    capacity: usize,
}

#[derive(Debug)]
pub enum PushError {
    Full,
}

impl SpscQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, data: &[u8]) -> Result<(), PushError> {
        let required = 4 + data.len();
        if self.buffer.len() + required > self.capacity {
            return Err(PushError::Full);
        }
        let len = data.len() as u32;
        self.buffer.extend(len.to_ne_bytes());
        self.buffer.extend(data.iter().copied());
        Ok(())
    }

    pub fn pop_all(&mut self) -> Vec<u8> {
        let data: Vec<u8> = self.buffer.drain(..).collect();
        data
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}
