use crate::hazard_ptr;

use super::entry::Entry;

/// TODO
#[derive(Debug)]
pub struct RefValue<K, V> {
    pub(crate) guard: hazard_ptr::Guard<Entry<K, V>>,
}

impl<K, V> RefValue<K, V> {
    /// TODO
    pub fn value(&self) -> &V {
        &self.guard.value
    }
}

impl<K, V> AsRef<V> for RefValue<K, V> {
    fn as_ref(&self) -> &V {
        self.value()
    }
}

impl<K, V> PartialEq for RefValue<K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value().eq(other.value())
    }
}

impl<K, V> PartialEq<V> for RefValue<K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &V) -> bool {
        self.value().eq(other)
    }
}
