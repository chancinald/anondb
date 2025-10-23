#[cfg(feature = "redb")]
mod kv_redb;
mod lexicographic;
mod sort;

#[cfg(test)]
mod test;

#[cfg(feature = "redb")]
pub use kv_redb::*;
pub use lexicographic::*;
pub use sort::*;

use std::ops::RangeBounds;

use anyhow::Result;
use serde::Deserialize;

/// A standard interface for accessing entries in the KV.
pub trait OpaqueItem {
    fn key(&self) -> &[u8];
    fn value(&self) -> &[u8];
}

/// A generic key-value store. Assumed to be capable of transactional mutation of key-value collections.
pub trait KV: Sized + ReadOperations + WriteOperations {
    type ReadTransaction: ReadOperations;
    type WriteTransaction: WriteTx;

    /// Initialize a kv persisted to a path. What path is (directory, file, etc) is determined by
    /// the underlying implementation.
    fn at_path(path: &std::path::Path) -> Result<Self>;
    /// Initialize a kv with a byte representation of the initial state. This byte
    /// representation is arbitrary to the concrete implementation.
    fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self>;
    fn write_tx(&self) -> Result<Self::WriteTransaction>;
    fn read_tx(&self) -> Result<Self::ReadTransaction>;

    /// Iterate over the contents of a collection, in ascending lexicographic order. Must be
    /// `O(N)`.
    fn scan<S>(&self, table: &str, predicate: S) -> Result<()>
    where
        S: Fn(&[u8], &[u8]) -> Result<bool>;
}

pub trait ReadOperations {
    /// Determine the number of keys present in a table.
    fn count(&self, table: &str) -> Result<u64>;
    /// Retrieve the value associated to a key for a table. Must be `O(1)`.
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Retrieve an iterator over a range of keys. Returns a reference to each key and value.
    fn range<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a>;

    /// Determine the number of keys present in a multimap table.
    fn count_multimap(&self, table: &str) -> Result<u64>;
    /// Retrieve values associated with a key in a multimap table.
    fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>>;

    fn range_multimap<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a>;

    fn range_buffered<'a, T: for<'de> Deserialize<'de>>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
        selector: impl Fn(&[u8], &[u8], &mut dyn FnMut()) -> Result<Option<T>>,
    ) -> Result<Vec<T>> {
        let mut is_done = false;
        let mut out = Vec::default();
        for item in self.range(table, range)? {
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

    fn range_buffered_multimap<'a, T: for<'de> Deserialize<'de>>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
        selector: impl Fn(&[u8], &[u8], &mut dyn FnMut()) -> Result<Option<T>>,
    ) -> Result<Vec<T>> {
        let mut is_done = false;
        let mut out = Vec::default();
        for item in self.range_multimap(table, range)? {
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

pub trait WriteOperations {
    /// Insert a key for a multimap table. Must be `O(1)`.
    fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()>;
    /// Remove a key and value from a multimap table. Returns `true` if the key/value pair was
    /// present in the table. Must be `O(1)`.
    fn remove_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool>;
    /// Remove all values associated with a key in a multimap table. Must be `O(1)`.
    fn remove_all_multimap(&self, table: &str, key: &[u8]) -> Result<()>;

    /// Insert a key for a table and return the old value if it exists. Must be `O(1)`.
    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Remove a key from a table. Must be `O(1)`.
    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
    /// Remove all entries in a table.
    fn clear(&self, table: &str) -> Result<()>;
    /// Remove all entries in a multimap table.
    fn clear_multimap(&self, table: &str) -> Result<()>;
}

pub trait WriteTx: ReadOperations + WriteOperations {
    fn commit(self) -> Result<()>;
}

#[cfg(test)]
pub fn rand_utf8(len: usize) -> String {
    vec![char::default(); len]
        .into_iter()
        .map(|_| rand::random::<char>())
        .collect()
}

impl<T: KV> WriteOperations for T {
    fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.insert_multimap(table, key, value)?;
        tx.commit()?;
        Ok(())
    }

    fn remove_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        let removed = tx.remove_multimap(table, key, value)?;
        tx.commit()?;
        Ok(removed)
    }

    fn remove_all_multimap(&self, table: &str, key: &[u8]) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.remove_all_multimap(table, key)?;
        tx.commit()?;
        Ok(())
    }

    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        let existing_maybe = tx.insert(table, key, value)?;
        tx.commit()?;
        Ok(existing_maybe)
    }

    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        let removed = tx.remove(table, key)?;
        tx.commit()?;
        Ok(removed)
    }

    fn clear(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.clear(table)?;
        tx.commit()?;
        Ok(())
    }

    fn clear_multimap(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.clear_multimap(table)?;
        tx.commit()?;
        Ok(())
    }
}
