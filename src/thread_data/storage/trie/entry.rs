use alloc::boxed::Box;
use core::sync::atomic;

use super::{CustomPtr, Level, PtrTarget};

#[derive(Debug)]
pub struct Entry<T> {
    key: u64,
    data: T,
    pub next: CustomPtr<T>,
}

impl<T> Entry<T> {
    /// Creates a new simply Entry
    pub fn new(key: u64, data: T, next: CustomPtr<T>) -> Self {
        Self { key, data, next }
    }

    /// Get the Key for the Entry
    pub fn key(&self) -> u64 {
        self.key
    }
    /// Get a reference to the Data
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Attempts to load the Data for the given Key from the current Entry or
    /// the Entries in its Chain
    pub fn get_chain(&self, key: u64, current_level: &Level<T>) -> Option<&T> {
        // Loop over all the Entries in the Chain of Entries, starting with the
        // current one
        let mut current = self;
        loop {
            // If the Key of the current Entry matches the Key we are looking
            // for, we found the correct Entry, so we should just return the
            // Data for that Entry
            if current.key == key {
                return Some(&current.data);
            }

            // Load the Pointer to the next Element in the Chain
            match current.next.load(atomic::Ordering::Acquire) {
                PtrTarget::Entry(entry_ptr) => {
                    // If it points an Entry, we will simply store that entry
                    // as our current one for the next iteration of the loop
                    current = unsafe { &*entry_ptr };
                }
                PtrTarget::Level(sub_lvl_ptr) => {
                    let sub_lvl = unsafe { &*sub_lvl_ptr };

                    // Check if the Level we are pointing to is the current
                    // Level we started on, if that is the Case it means we
                    // reached the End of the Chain and therefore could not
                    // find the Key and should return None
                    if sub_lvl.level() == current_level.level() {
                        return None;
                    }

                    // Continue the search for the Key on the other Level we
                    // just found and return whatever it finds or does not find
                    return sub_lvl.get(key);
                }
            };
        }
    }

    /// Inserts a new Entry on the current Chain of Entries
    pub fn insert_chain(&self, mut new_entry: Box<Self>, level: &Level<T>) -> &T {
        let mut current = self;
        let mut pos = 1;

        // Iterate over the Entries currently on the Chain
        loop {
            // If the current Entry's key matches the new Key, we panic as this
            // is should never happen
            if current.key == new_entry.key {
                panic!("The Same key should never be inserted twice");
            }

            // Load the next Pointer and check if it points to a level
            if let PtrTarget::Level(sub_lvl_ptr) = current.next.load(atomic::Ordering::Acquire) {
                let sub_lvl = unsafe { &*sub_lvl_ptr };

                // If the Level it points, is the same Level we started on we
                // know that we reached the End and should either append the
                // new Entry to the Chain or create a new Level, depending on
                // the current Chain-Length
                if sub_lvl.level() == level.level() {
                    let expected = PtrTarget::Level(sub_lvl_ptr);

                    // Check if the Chain has reached or exceeded its maximum
                    // Length
                    if pos >= level.max_chain() {
                        // Create the next Level
                        let n_level = level.create_next();
                        let n_level_ptr = Box::into_raw(n_level);

                        // Attempt to replace the next Pointer with the pointer
                        // to the new Level
                        match current.next.compare_exchange(
                            expected,
                            PtrTarget::Level(n_level_ptr),
                            atomic::Ordering::AcqRel,
                            atomic::Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                // If we successfully added the new Level, we
                                // will start to move all the Entries from the
                                // Bucket over to the new Level
                                level.move_entries_to_new_level(new_entry.key, n_level_ptr);
                            }
                            Err(_) => {
                                // If we failed to add the new Level, we simply
                                // deallocate it again and continue with the
                                // loop
                                let boxed_lvl = unsafe { Box::from_raw(n_level_ptr) };
                                drop(boxed_lvl);
                            }
                        }
                    } else {
                        // Store the pointer of the current Level as the next
                        // Pointer of the new Entry as it will be the new end
                        // of the Chain
                        new_entry.next.store(
                            PtrTarget::Level(level.get_own_ptr()),
                            atomic::Ordering::Release,
                        );
                        let n_entry_ptr = Box::into_raw(new_entry);

                        // Attempt to append the new Entry to the Queue
                        match current.next.compare_exchange(
                            expected,
                            PtrTarget::Entry(n_entry_ptr),
                            atomic::Ordering::AcqRel,
                            atomic::Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                // If it worked simply return a reference to
                                // the Data of the Entry
                                let entry = unsafe { &*n_entry_ptr };
                                return entry.data();
                            }
                            Err(_) => {
                                // If this did not work, recover the new Entry
                                // and continue with the loop
                                new_entry = unsafe { Box::from_raw(n_entry_ptr) };
                            }
                        };
                    }
                }
            }

            // Load the next Pointer of the current Entry
            match current.next.load(atomic::Ordering::Acquire) {
                PtrTarget::Entry(entry_ptr) => {
                    // If it also is an Entry, simply load it as the current
                    // Entry for the loop and increment our position to keep
                    // track of the Length of the Chain
                    current = unsafe { &*entry_ptr };
                    pos += 1;
                }
                PtrTarget::Level(sub_lvl_ptr) => {
                    // If it points to an Entry, load it and make sure the
                    // Level we load is the one following the current one
                    let mut sub_lvl = unsafe { &*sub_lvl_ptr };
                    while sub_lvl.previous() != level.get_own_ptr() {
                        sub_lvl = unsafe { &*sub_lvl.previous() };
                    }

                    // Attempt to insert the new Entry on the loaded Queue
                    // and return whatever it returns
                    return sub_lvl.insert_level(new_entry);
                }
            };
        }
    }

    /// Cleans up the Entry and all the other Parts in it's Chain
    pub fn drop_entry(self, level_ptr: *mut Level<T>) {
        // Load the next Element in the Chain
        match self.next.load(atomic::Ordering::Acquire) {
            PtrTarget::Level(sub_lvl_ptr) => {
                // Check that the next Level does not point to our current
                // Level, otherwise return as we have reached the End of our
                // current Chain
                if sub_lvl_ptr == level_ptr {
                    return;
                }

                // Load the next Level and then simply drop it for cleanup
                unsafe { Box::from_raw(sub_lvl_ptr) };
            }
            PtrTarget::Entry(entry_ptr) => {
                // Load the next Entry and then recursively call this function
                // on that next Entry
                let boxed = unsafe { Box::from_raw(entry_ptr) };
                boxed.drop_entry(level_ptr);
            }
        };
    }
}
