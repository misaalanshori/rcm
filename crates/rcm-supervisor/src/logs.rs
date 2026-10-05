use std::collections::VecDeque;
use std::sync::Mutex;

pub struct LogRingBuffer {
    capacity: usize,
    buffer: Mutex<VecDeque<String>>,
}

impl LogRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn push(&self, line: impl Into<String>) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(line.into());
    }

    pub fn get_recent(&self) -> Vec<String> {
        let buf = self.buffer.lock().unwrap();
        buf.iter().cloned().collect()
    }

    pub fn clear(&self) {
        let mut buf = self.buffer.lock().unwrap();
        buf.clear();
    }
}

impl Default for LogRingBuffer {
    fn default() -> Self {
        Self::new(5000)
    }
}
