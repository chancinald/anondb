#[cfg(feature = "redb")]
mod kv_redb;
mod lexicographic;

#[cfg(feature = "redb")]
pub use kv_redb::*;
pub use lexicographic::*;
use serde::Deserialize;

use std::ops::Bound;
use std::ops::RangeBounds;

use anyhow::Result;

#[derive(Debug, Clone, Default, PartialEq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl ToString for SortDirection {
    fn to_string(&self) -> String {
        match self {
            Self::Asc => "asc".into(),
            Self::Desc => "desc".into(),
        }
    }
}

pub struct KeyRange<T> {
    pub start: Bound<T>,
    pub end: Bound<T>,
}

impl<T> RangeBounds<T> for KeyRange<T> {
    fn start_bound(&self) -> Bound<&T> {
        self.start.as_ref()
    }

    fn end_bound(&self) -> Bound<&T> {
        self.end.as_ref()
    }
}

/// A vector of bytes representing a lexicographically sortable set of keys. Each key is separator
/// by a byte 0x00 to allow partial index searches.
///
/// Ff i have an index (id: u8, created_at: u8, name: String ) and i want to filter by
/// { id = 0, created_at = gt(1) && lt(99) }
///
/// I need to sort by 00000000100000000..0000000063000000. But i need to include all keys that are
/// longer than the provided slice. e.g. 0000000050000000a3eb398e should be included.
///
/// To achieve this we need a separator that is a fixed value that we can use for comparison. If we
/// choose this byte as 0x00, then we can suffix our sort queries with 0x01 to include all longer
/// keys.
///
/// This strategy adds ~1 byte of overhead per field (0 bytes for indices with 1 field).
#[derive(Default, Clone)]
pub struct LexicographicKey {
    pub bytes: Vec<u8>,
}

impl LexicographicKey {
    /// Append a slice representing a lexicographically sortable key.
    pub fn append_key_slice(&mut self, slice: &[u8]) {
        if !self.bytes.is_empty() {
            self.append_separator();
        }
        self.bytes.extend_from_slice(slice);
    }

    /// Append a 0x01 byte that will sort all longer keys before this key.
    pub fn append_upper_inclusive_byte(&mut self) {
        self.bytes.push(0x01);
    }

    pub fn append_separator(&mut self) {
        self.bytes.push(0x00);
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}

pub trait OpaqueItem {
    fn key(&self) -> &[u8];
    fn value(&self) -> &[u8];
}

/// A generic key-value store. Assumed to be capable of transactional mutation of key-value collections.
pub trait KV: Sized + Operations {
    type Transaction: Transaction;
    /// Initialize a kv persisted to a path. What path is (directory, file, etc) is determined by
    /// the underlying implementation.
    fn at_path(path: &std::path::Path) -> Result<Self>;
    /// Initialize a kv with a byte representation of the initial state. This byte
    /// representation is arbitrary to the concrete implementation.
    fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self>;
    fn write_tx(&self) -> Result<Self::Transaction>;
    fn read_tx(&self) -> Result<Self::Transaction>;

    /// Iterate over the contents of a collection, in ascending lexicographic order. Must be
    /// `O(N)`.
    fn scan<S>(&self, table: &str, predicate: S) -> Result<()>
    where
        S: Fn(&[u8], &[u8]) -> Result<bool>;
}

pub trait Operations {
    /// Insert a key for a multimap table. Must be `O(1)`.
    fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()>;
    /// Remove a key and value from a multimap table. Returns `true` if the key/value pair was
    /// present in the table. Must be `O(1)`.
    fn remove_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool>;
    /// Remove all values associated with a key in a multimap table. Must be `O(1)`.
    fn remove_all_multimap(&self, table: &str, key: &[u8]) -> Result<()>;
    /// Get an iterator over all the values associated with a key in a multimap table.
    fn get_multimap(&self, table: &str, key: &[u8]) -> Result<impl Iterator<Item = &[u8]>>;

    /// Insert a key for a table and return the old value if it exists. Must be `O(1)`.
    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Remove a key from a table. Must be `O(1)`.
    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Retrieve the value associated to a key for a table. Must be `O(1)`.
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Determine the number of keys present in a table.
    fn count(&self, table: &str) -> Result<u64>;
    /// Remove all entries in the table.
    fn clear(&self, table: &str) -> Result<()>;

    /// Invoke a handler function on a range of entries in a collection. The handler function
    /// returns an optional value, and a boolean indicating whether to continue iteration.
    /// Values returned by the handler are returned as an Iterator.
    fn range<'a>(
        &self,
        table: String,
        is_multimap: bool,
        range: impl RangeBounds<Vec<u8>> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>>;

    fn range_buffered<'a, T: for<'de> Deserialize<'de>>(
        &self,
        table: &str,
        is_multimap: bool,
        range: impl RangeBounds<Vec<u8>> + 'a,
        selector: impl Fn(&[u8], &[u8], &mut dyn FnMut()) -> Result<Option<T>>,
    ) -> Result<Vec<T>> {
        let mut is_done = false;
        let mut out = Vec::default();
        for item in self.range(table.to_string(), is_multimap, range)? {
            let item = item?;
            if let Some(item) = selector(item.key(), item.value(), &mut || {
                is_done = true;
            })? {
                out.push(item);
            }
            if is_done {
                break;
            }
        }
        Ok(out)
    }
}

pub trait Transaction: Operations {
    fn commit(self) -> Result<()>;
}
