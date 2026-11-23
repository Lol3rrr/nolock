use crate::thread_data::storage::trie::{entry::Entry, level::Level};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PtrTarget<T> {
    Entry(*mut Entry<T>),
    Level(*mut Level<T>),
}

const ENTRY_MASK: usize = 0b1;
const CLEAN_PTR_MASK: usize = usize::MAX - 1;

impl<T> From<*mut ()> for PtrTarget<T> {
    fn from(raw_ptr: *mut ()) -> Self {
        if (raw_ptr.addr() & ENTRY_MASK) > 0 {
            PtrTarget::Entry(raw_ptr.map_addr(|a| a & CLEAN_PTR_MASK).cast())
        } else {
            PtrTarget::Level(raw_ptr.cast())
        }
    }
}
impl<T> From<PtrTarget<T>> for *mut () {
    fn from(target: PtrTarget<T>) -> Self {
        match target {
            PtrTarget::Entry(raw) => raw.map_addr(|a| a | ENTRY_MASK).cast(),
            PtrTarget::Level(raw) => raw.cast(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_from_level() {
        let input = PtrTarget::<usize>::Level(core::ptr::without_provenance_mut::<Level<usize>>(0x100));
        let raw_ptr: *mut () = input.into();
        assert_eq!(input, PtrTarget::from(raw_ptr));
    }
    #[test]
    fn into_from_entry() {
        let input = PtrTarget::<usize>::Entry(core::ptr::without_provenance_mut::<Entry<usize>>(0x100));
        let raw_ptr: *mut () = input.into();
        assert_eq!(input, PtrTarget::from(raw_ptr));
    }
}
