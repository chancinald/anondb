mod iter;
mod maybe;
mod tx;

use iter::*;
use maybe::*;
use tx::*;

use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Result;

use redb::*;

use super::*;

fn tabledef(name: &str) -> TableDefinition<&'static [u8], &'static [u8]> {
    TableDefinition::new(name)
}

fn tabledef_multimap(name: &str) -> MultimapTableDefinition<&'static [u8], &'static [u8]> {
    MultimapTableDefinition::new(name)
}

pub struct RedbKV {
    db: Database,
}

impl RedbKV {
    fn read(&self) -> Result<ReadTransaction> {
        Ok(self.db.begin_read()?)
    }

    fn write(&self) -> Result<WriteTransaction> {
        Ok(self.db.begin_write()?)
    }
}

impl KV for RedbKV {
    type ReadTransaction = RedbReadTransaction;
    type WriteTransaction = RedbWriteTransaction;

    fn at_path(_path: &std::path::Path) -> Result<Self> {
        unimplemented!()
    }

    fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self> {
        let mem_backend = redb::backends::InMemoryBackend::new();
        if let Some(bytes) = bytes_maybe {
            mem_backend.write(0, bytes)?;
        }
        Ok(Self {
            db: Database::builder().create_with_backend(mem_backend)?,
        })
    }

    fn scan<S>(&self, table: &str, predicate: S) -> Result<()>
    where
        S: Fn(&[u8], &[u8]) -> Result<bool>,
    {
        for item in self.range(table.to_string(), ..)? {
            let item = item?;
            if !predicate(item.key(), item.value())? {
                break;
            }
        }
        Ok(())
    }

    fn write_tx(&self) -> Result<Self::WriteTransaction> {
        Ok(RedbWriteTransaction {
            write: self.db.begin_write()?,
        })
    }

    fn read_tx(&self) -> Result<Self::ReadTransaction> {
        Ok(RedbReadTransaction {
            read: self.db.begin_read()?,
            tables: RwLock::new(HashMap::default()),
        })
    }
}

impl WriteOperations for RedbKV {
    fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.insert_multimap(table, key, value)
    }

    fn remove_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.remove_multimap(table, key, value)
    }

    fn remove_all_multimap(&self, table: &str, key: &[u8]) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.remove_all_multimap(table, key)
    }

    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.insert(table, key, value)
    }

    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.remove(table, key)
    }

    fn clear(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.clear(table)
    }

    fn clear_multimap(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx()?;
        tx.clear_multimap(table)
    }
}

/// Operations occuring outside of a transaction. "One and done" operations.
impl ReadOperations for RedbKV {
    fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        let table = tx.read.open_multimap_table(tabledef_multimap(table))?;
        let inner_iter = table.get(key)?;
        Ok(RedbReadIter {
            data: Arc::new(key.to_vec()),
            inner_iter,
            map_fn: |key, item| {
                let val = item?;
                Ok(RedbItem {
                    item: (key.into(), val.into()),
                })
            },
        })
    }

    fn count(&self, table: &str) -> Result<u64> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        tx.count(table)
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        tx.get(table, key)
    }

    fn range<'a>(
        &'a self,
        table: String,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        let table = tx.read.open_table(tabledef(&table))?;
        let inner_iter = table.range(range)?;
        Ok(RedbReadIter {
            data: Arc::new(()),
            inner_iter,
            map_fn: |_data, item| {
                let (k, v) = item?;
                Ok(RedbItem {
                    item: (k.into(), v.into()),
                })
            },
        })
    }

    fn range_multimap<'a>(
        &'a self,
        table: String,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        let table = tx.read.open_multimap_table(tabledef_multimap(&table))?;
        let inner_iter = FlatMapFallible::from(table.range(range)?.map(|v| {
            let (key, vals) = v?;
            let key = Arc::new(key);
            Ok(vals.map(move |v| Ok((key.clone(), v?))))
        }));
        Ok(RedbReadIter {
            data: Arc::new(()),
            inner_iter,
            map_fn: |_data, item| {
                let (k, v) = item?;
                Ok(RedbItem {
                    item: (k.into(), v.into()),
                })
            },
        })
    }
}
