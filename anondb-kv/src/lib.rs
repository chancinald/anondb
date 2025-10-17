#[cfg(feature = "redb")]
mod kv_redb;
mod lexicographic;

#[cfg(feature = "redb")]
pub use kv_redb::*;
pub use lexicographic::*;

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

/// A generic key-value store. Assumed to be capable of transactional mutation of key-value collections.
pub trait KV: Sized + Operations {
    /// Initialize a kv persisted to a path. What path is (directory, file, etc) is determined by
    /// the underlying implementation.
    fn at_path(path: &std::path::Path) -> Result<Self>;
    /// Initialize a kv with a byte representation of the initial state. This byte
    /// representation is arbitrary to the concrete implementation.
    fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self>;
    fn write_tx(&self) -> Result<impl Transaction>;
    fn read_tx(&self) -> Result<impl Transaction>;

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
}

pub trait Transaction: Operations {
    fn commit(self) -> Result<()>;
}
