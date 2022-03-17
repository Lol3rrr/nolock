use core::sync::atomic;

use super::Level;

mod target;
pub use target::*;

#[derive(Debug)]
pub struct CustomPtr<T> {
    ptr: atomic::AtomicPtr<()>,
    _marker: core::marker::PhantomData<T>,
}

impl<T> CustomPtr<T> {
    pub fn new_level(ptr: *mut Level<T>) -> Self {
        let n_ptr = PtrTarget::Level(ptr);

        Self {
            ptr: atomic::AtomicPtr::new(n_ptr.into()),
            _marker: core::marker::PhantomData::default(),
        }
    }

    pub fn load(&self, order: atomic::Ordering) -> PtrTarget<T> {
        let value = self.ptr.load(order);
        PtrTarget::from(value)
    }

    pub fn store(&self, value: PtrTarget<T>, order: atomic::Ordering) {
        self.ptr.store(value.into(), order);
    }

    pub fn compare_exchange(
        &self,
        expected: PtrTarget<T>,
        new: PtrTarget<T>,
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
