use super::descriptor::Descriptor;

mod storage;

// TODO
// This can be implemented by copying and adapting the Trie-Structure
// like we already have in the ThreadData module.
//
// # Adaptations:
// * Use atomic-Ptrs as Data
// * Allow Data entries to be overwritten
// * Make it None-Generic (concrete *mut Descriptor Type as Data)

#[derive(Debug)]
pub struct PageMap {
    descriptors: storage::Collection,
}

impl PageMap {
    pub const fn new() -> Self {
        Self {
            descriptors: storage::Collection::new(),
        }
    }

    pub fn register_descriptor(&self, descriptor: *mut Descriptor) {
        self.descriptors.insert(descriptor);
    }
    pub fn unregister_descriptor(&self, descriptor: *mut Descriptor) {
        self.descriptors.remove(descriptor);
    }

    pub fn load_descriptor(&self, ptr: *mut u8) -> Option<*mut Descriptor> {
        self.descriptors.get(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let map = PageMap::new();
        drop(map);
    }

    #[test]
    fn register_load() {
        let map = PageMap::new();

        let initial_desc = Box::new(Descriptor::new(128, 4, Some(0), 0x0000 as *mut u8));
        let initial_desc_ptr = Box::into_raw(initial_desc);

        map.register_descriptor(initial_desc_ptr);

        let expected = Some(initial_desc_ptr);
        let result = map.load_descriptor(0x0000 as *mut u8);

        assert_eq!(expected, result);
    }
}
