use std::sync::Arc;

use redb::*;

use super::*;

/// An opaque reference to a redb item.
pub struct RedbItem<'a> {
    pub item: (MaybeGuarded<'a>, MaybeGuarded<'a>),
}

impl<'a> OpaqueItem for RedbItem<'a> {
    fn key(&self) -> &[u8] {
        self.item.0.value()
    }

    fn value(&self) -> &[u8] {
        self.item.1.value()
    }
}

/// Get a common reference to data in different locations.
pub enum MaybeOwned<'a, T> {
    Borrowed(&'a T),
    Owned(T),
    Arc(Arc<T>),
}

impl<'a, T> MaybeOwned<'a, T> {
    pub fn as_ref(&self) -> &T {
        match self {
            MaybeOwned::Borrowed(r) => r,
            MaybeOwned::Owned(t) => t,
            MaybeOwned::Arc(t) => t,
        }
    }
}

impl<'a, T> From<T> for MaybeOwned<'a, T> {
    fn from(value: T) -> Self {
        MaybeOwned::Owned(value)
    }
}

impl<'a, T> From<&'a T> for MaybeOwned<'a, T> {
    fn from(value: &'a T) -> Self {
        MaybeOwned::Borrowed(value)
    }
}

impl<'a, T> From<Arc<T>> for MaybeOwned<'a, T> {
    fn from(value: Arc<T>) -> Self {
        MaybeOwned::Arc(value)
    }
}

/// Get a common reference to bytes with different ownership models.
pub enum MaybeGuarded<'a> {
    Guarded(MaybeOwned<'a, AccessGuard<'a, &'static [u8]>>),
    Owned(Vec<u8>),
    Ref(&'a [u8]),
    Arc(Arc<Vec<u8>>),
}

impl<'a> MaybeGuarded<'a> {
    pub fn value(&self) -> &[u8] {
        match self {
            MaybeGuarded::Guarded(v) => v.as_ref().value(),
            MaybeGuarded::Owned(v) => v.as_slice(),
            MaybeGuarded::Ref(v) => v,
            MaybeGuarded::Arc(v) => v.as_ref(),
        }
    }
}

impl<'a> From<Arc<AccessGuard<'a, &'static [u8]>>> for MaybeGuarded<'a> {
    fn from(value: Arc<AccessGuard<'a, &'static [u8]>>) -> Self {
        MaybeGuarded::Guarded(MaybeOwned::Arc(value.clone()))
    }
}

impl<'a> From<Vec<u8>> for MaybeGuarded<'a> {
    fn from(value: Vec<u8>) -> Self {
        MaybeGuarded::Owned(value)
    }
}

impl<'a> From<Arc<Vec<u8>>> for MaybeGuarded<'a> {
    fn from(value: Arc<Vec<u8>>) -> Self {
        MaybeGuarded::Arc(value)
    }
}

impl<'a> From<&'a [u8]> for MaybeGuarded<'a> {
    fn from(value: &'a [u8]) -> Self {
        MaybeGuarded::Ref(value)
    }
}

impl<'a> From<AccessGuard<'a, &'static [u8]>> for MaybeGuarded<'a> {
    fn from(value: AccessGuard<'a, &'static [u8]>) -> Self {
        MaybeGuarded::Guarded(value.into())
    }
}

impl<'a> From<&'a AccessGuard<'a, &'static [u8]>> for MaybeGuarded<'a> {
    fn from(value: &'a AccessGuard<'a, &'static [u8]>) -> Self {
        MaybeGuarded::Guarded(value.into())
    }
}
