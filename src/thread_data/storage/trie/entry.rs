use std::sync::atomic;

use super::{CustomPtr, Level, PtrTarget};

#[derive(Debug)]
pub struct Entry<T> {
    key: u64,
    data: T,
    pub next: CustomPtr<T>,
}

impl<T> Entry<T> {
    pub fn new(key: u64, data: T, next: CustomPtr<T>) -> Self {
        Self { key, data, next }
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn into_data(self) -> T {
        self.data
    }

    pub fn get_chain(&self, key: u64, current_level: &Level<T>) -> Option<&T> {
        if self.key == key {
            return Some(&self.data);
        }

        match self.next.load(atomic::Ordering::SeqCst) {
            PtrTarget::Entry(entry_ptr) => {
                let entry = unsafe { &*entry_ptr };
                entry.get_chain(key, current_level)
            }
            PtrTarget::Level(sub_lvl_ptr) => {
                let sub_lvl = unsafe { &*sub_lvl_ptr };

                if sub_lvl.level() == current_level.level() {
                    return None;
                }

                sub_lvl.get(key)
            }
        }
    }

    pub fn insert_chain(&self, key: u64, mut data: T, level: &Level<T>, pos: usize) {
        if self.key == key {
            panic!("The Same key should never be inserted twice");
        }

        if let PtrTarget::Level(sub_lvl_ptr) = self.next.load(atomic::Ordering::SeqCst) {
            let sub_lvl = unsafe { &*sub_lvl_ptr };

            if sub_lvl.level() == level.level() {
                let expected = PtrTarget::Level(sub_lvl_ptr);

                if pos == level.max_chain() {
                    let n_level =
                        Level::new(level.level() + 1, level.key_size(), level.get_own_ptr());
                    let n_level_ptr = Box::into_raw(n_level);

                    match self.next.compare_exchange(
                        &expected,
                        &PtrTarget::Level(n_level_ptr),
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => {
                            level.move_buckets_to_new_level(key, n_level_ptr);
                        }
                        Err(_) => {}
                    };
                } else {
                    let n_entry_ptr = Box::into_raw(Box::new(Self::new(
                        key,
                        data,
                        CustomPtr::new_level(level.get_own_ptr()),
                    )));

                    match self.next.compare_exchange(
                        &expected,
                        &PtrTarget::Entry(n_entry_ptr),
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => return,
                        Err(_) => {
                            let boxed = unsafe { Box::from_raw(n_entry_ptr) };
                            data = boxed.into_data();
                        }
                    };
                }
            }
        }

        match self.next.load(atomic::Ordering::SeqCst) {
            PtrTarget::Entry(entry_ptr) => {
                let entry = unsafe { &*entry_ptr };
                entry.insert_chain(key, data, level, pos + 1);
            }
            PtrTarget::Level(sub_lvl_ptr) => {
                let mut sub_lvl = unsafe { &*sub_lvl_ptr };
                while sub_lvl.previous() != level.get_own_ptr() {
                    sub_lvl = unsafe { &*sub_lvl.previous() };
                }

                sub_lvl.insert_level(key, data);
            }
        };
    }

    pub fn drop_entry(self: Box<Self>, level_ptr: *mut Level<T>) {
        match self.next.load(atomic::Ordering::SeqCst) {
            PtrTarget::Level(sub_lvl_ptr) => {
                if sub_lvl_ptr == level_ptr {
                    return;
                }

                unsafe { Box::from_raw(sub_lvl_ptr) };
            }
            PtrTarget::Entry(entry_ptr) => {
                let boxed = unsafe { Box::from_raw(entry_ptr) };
                boxed.drop_entry(level_ptr);
            }
        };
    }
}
