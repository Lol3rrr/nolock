use std::sync::atomic;

use super::{Entry, Level};

#[derive(Debug)]
pub struct CustomPtr<T> {
    ptr: atomic::AtomicPtr<()>,
    _marker: std::marker::PhantomData<T>,
}

#[derive(Debug)]
pub enum PtrTarget<T> {
    Entry(*mut Entry<T>),
    Level(*mut Level<T>),
}

impl<T> From<*mut ()> for PtrTarget<T> {
    fn from(raw_ptr: *mut ()) -> Self {
        const ENTRY_CHECK_MASK: usize = 0b1;
        const CLEAN_MASK: usize = usize::MAX - 1;

        if (raw_ptr as usize & ENTRY_CHECK_MASK) > 0 {
            let ptr = (raw_ptr as usize & CLEAN_MASK) as *mut ();
            PtrTarget::Entry(ptr as *mut Entry<T>)
        } else {
            PtrTarget::Level(raw_ptr as *mut Level<T>)
        }
    }
}
impl<T> Into<*mut ()> for &PtrTarget<T> {
    fn into(self) -> *mut () {
        match self {
            PtrTarget::Entry(raw) => (*raw as usize | 0b1) as *mut (),
            PtrTarget::Level(raw) => *raw as *mut (),
        }
    }
}

impl<T> CustomPtr<T> {
    pub fn new_level(ptr: *mut Level<T>) -> Self {
        let n_ptr = ptr;

        Self {
            ptr: atomic::AtomicPtr::new(n_ptr as *mut ()),
            _marker: std::marker::PhantomData::default(),
        }
    }

    pub fn load(&self, order: atomic::Ordering) -> PtrTarget<T> {
        let value = self.ptr.load(order);
        PtrTarget::from(value)
    }

    pub fn store(&self, value: &PtrTarget<T>, order: atomic::Ordering) {
        self.ptr.store(value.into(), order);
    }

    pub fn compare_exchange(
        &self,
        expected: &PtrTarget<T>,
        new: &PtrTarget<T>,
        sucess: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<PtrTarget<T>, PtrTarget<T>> {
        match self
            .ptr
            .compare_exchange(expected.into(), new.into(), sucess, failure)
        {
            Ok(raw) => Ok(PtrTarget::from(raw)),
            Err(raw) => Err(PtrTarget::from(raw)),
        }
    }
}
