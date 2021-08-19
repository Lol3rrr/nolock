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
        if (raw_ptr as usize & ENTRY_MASK) > 0 {
            let ptr = (raw_ptr as usize & CLEAN_PTR_MASK) as *mut ();
            PtrTarget::Entry(ptr as *mut Entry<T>)
        } else {
            PtrTarget::Level(raw_ptr as *mut Level<T>)
        }
    }
}
impl<T> From<PtrTarget<T>> for *mut () {
    fn from(target: PtrTarget<T>) -> Self {
        match target {
            PtrTarget::Entry(raw) => (raw as usize | ENTRY_MASK) as *mut (),
            PtrTarget::Level(raw) => raw as *mut (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_from_level() {
        let input = PtrTarget::<usize>::Level(0x100 as *mut Level<usize>);
        let raw_ptr: *mut () = input.into();
        assert_eq!(input, PtrTarget::from(raw_ptr));
    }
    #[test]
    fn into_from_entry() {
        let input = PtrTarget::<usize>::Entry(0x100 as *mut Entry<usize>);
        let raw_ptr: *mut () = input.into();
        assert_eq!(input, PtrTarget::from(raw_ptr));
    }
}
