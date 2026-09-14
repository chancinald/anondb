mod kv_memory;
#[cfg(feature = "redb")]
mod kv_redb;

#[cfg(test)]
mod test;

use std::ops::RangeBounds;

pub use kv_memory::*;
#[cfg(feature = "redb")]
pub use kv_redb::*;

pub(crate) use log_ext as log;

use anyhow::Result;
use futures::Stream;

/// A standard interface for accessing entries in a KV.
pub trait OpaqueItem {
    fn key(&self) -> &[u8];
    fn value(&self) -> impl Future<Output = &[u8]>;
}

/// A generic key-value store. Assumed to be capable of transactional mutation of key-value collections.
pub trait KV:
    Sized
    + ReadOperations<Item = <Self as KV>::Item>
    + RangeReadOperations<Item = <Self as KV>::Item>
    + WriteOperations<Item = <Self as KV>::Item>
    + Clone
{
    type Item: OpaqueItem;
    type ReadTransaction: ReadOperations<Item = <Self as KV>::Item>
        + RangeReadOperations<Item = <Self as KV>::Item>;
    type WriteTransaction: WriteTx<Item = <Self as KV>::Item>;
    type Config;

    /// Initialize a kv persisted to a path. What path is (directory, file, etc) is determined by
    /// the underlying implementation.
    fn new(config_maybe: Option<Self::Config>) -> impl Future<Output = Result<Self>>;

    /// Begin a new write transaction on the kv.
    fn write_tx(&self) -> impl Future<Output = Result<Self::WriteTransaction>>;
    /// Begin a new read transaction on the kv.
    fn read_tx(&self) -> impl Future<Output = Result<Self::ReadTransaction>>;
}

pub trait ReadOperations {
    type Item: OpaqueItem;
    /// Determine the number of keys present in a table.
    fn count(&self, table: &str) -> impl Future<Output = Result<u64>>;

    /// Retrieve the value associated to a key for a table. Must be `O(1)`.
    fn get(&self, table: &str, key: &[u8]) -> impl Future<Output = Result<Option<Self::Item>>>;

    /// Retrieve the number of non-empty keys in a multimap table.
    fn count_multimap(&self, table: &str) -> impl Future<Output = Result<u64>>;

    /// Retrieve the number of entries corresponding to a single key in a multimap table.
    fn count_multimap_values(&self, table: &str, key: &[u8]) -> impl Future<Output = Result<u64>>;
}

pub trait WriteOperations {
    type Item: OpaqueItem;
    /// Insert a key for a table and return the old value if it exists. Must be `O(1)`.
    fn insert(
        &self,
        table: &str,
        key: &[u8],
        value: &[u8],
    ) -> impl Future<Output = Result<Option<Vec<u8>>>>;

    /// Remove a key from a table. Must be `O(1)`.
    fn remove(&self, table: &str, key: &[u8]) -> impl Future<Output = Result<Option<Vec<u8>>>>;

    /// Remove all entries in a table.
    fn clear(&self, table: &str) -> impl Future<Output = Result<()>>;

    fn rename_table(&self, table: &str, new_name: &str) -> impl Future<Output = Result<()>>;

    /// Insert a key into a table with multiple entries per key.
    fn insert_multimap(
        &self,
        table: &str,
        key: &[u8],
        value: &[u8],
    ) -> impl Future<Output = Result<()>>;

    /// Remove a key and value from a multimap table.
    fn remove_multimap_value(
        &self,
        table: &str,
        key: &[u8],
        value: &[u8],
    ) -> impl Future<Output = Result<bool>>;

    /// Remove a key from a multimap table.
    fn remove_multimap(&self, table: &str, key: &[u8]) -> impl Future<Output = Result<bool>>;

    /// Remove all entries in a multimap table.
    fn clear_multimap(&self, table: &str) -> impl Future<Output = Result<()>>;
}

pub trait RangeReadOperations {
    type Item: OpaqueItem;
    /// Get a range of entries in a table as an async stream.
    fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> impl Future<Output = Result<impl Stream<Item = Result<Self::Item>>>>;

    /// Get a range of entries in a multimap table as an async stream.
    fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> impl Future<Output = Result<impl Stream<Item = Result<Self::Item>>>>;

    /// Retrieve all values associated with a key in a multimap collection.
    fn get_multimap<'a>(
        &'a self,
        table: &'a str,
        key: &'a [u8],
    ) -> impl Future<Output = Result<impl Stream<Item = Result<Self::Item>> + 'a>>;
}

pub trait WriteTx: ReadOperations + WriteOperations {
    type Item: OpaqueItem;
    fn commit(self) -> impl Future<Output = Result<()>>;
}
