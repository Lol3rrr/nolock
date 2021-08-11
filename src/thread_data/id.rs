use std::{
    convert::TryInto,
    hash::{Hash, Hasher},
};

struct IDHasher {
    result: u64,
}
impl std::hash::Hasher for IDHasher {
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() == 8 {
            self.result = u64::from_le_bytes(bytes.try_into().unwrap());
            return;
        }

        println!("Bytes: {:?}", bytes);
    }
    fn finish(&self) -> u64 {
        self.result
    }
}

pub struct Id {
    thread_id: std::thread::ThreadId,
}

impl Id {
    pub fn new() -> Self {
        Self {
            thread_id: std::thread::current().id(),
        }
    }

    pub fn as_u64(&self) -> u64 {
        let mut hasher = IDHasher { result: 0 };

        self.thread_id.hash(&mut hasher);
        hasher.finish()
    }
}
