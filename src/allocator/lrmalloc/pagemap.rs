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

pub struct PageMap {
    descriptors: storage::Collection,
}

impl PageMap {
    pub fn new() -> Self {
        Self {
            descriptors: storage::Collection::new(),
        }
    }

    pub fn register_descriptor(&self, descriptor: *mut Descriptor) {
        self.descriptors.insert(descriptor);
    }
    pub fn unregister_descriptor(&self, descriptor: *mut Descriptor) {
        // TODO
        // Actually unregister the given Descriptor
    }

    pub fn load_descriptor(&self, ptr: *mut u8) -> *mut Descriptor {
        self.descriptors.get(ptr).expect("IDK")
    }
}
