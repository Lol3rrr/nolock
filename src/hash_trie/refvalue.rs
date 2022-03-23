use core::fmt::Debug;

use crate::hyaline;

use super::entry::Entry;

/// TODO
pub struct RefValue<'a, K, V> {
    pub(crate) entry_ptr: *const Entry<K, V>,
    pub(crate) _handle: hyaline::Handle<'a>,
}

impl<'a, K, V> Debug for RefValue<'a, K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("RefValue").field(self.value()).finish()
    }
}

impl<'a, K, V> RefValue<'a, K, V> {
    /// TODO
    pub fn value(&self) -> &V {
        unsafe { &(*self.entry_ptr).value }
    }
}

impl<'a, K, V> AsRef<V> for RefValue<'a, K, V> {
    fn as_ref(&self) -> &V {
        self.value()
    }
}

impl<'a, K, V> PartialEq for RefValue<'a, K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value().eq(other.value())
    }
}

impl<'a, K, V> PartialEq<V> for RefValue<'a, K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &V) -> bool {
        self.value().eq(other)
    }
}
