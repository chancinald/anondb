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
        for item in self.range(table, ..)? {
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
            multimap_tables: RwLock::new(HashMap::default()),
        })
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
        let table = match tx.read_multimap_table(table)? {
            Some(t) => t,
            None => return Ok(MaybeEmptyIter::default()),
        };
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
        }
        .into())
    }

    fn count_multimap(&self, table: &str) -> Result<u64> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        tx.count_multimap(table)
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
        table: &str,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        let table = match tx.read_table(&table)? {
            Some(t) => t,
            None => return Ok(MaybeEmptyIter::default()),
        };
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
        }
        .into())
    }

    fn range_multimap<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx()?;
        let table = match tx.read_multimap_table(&table)? {
            Some(t) => t,
            None => return Ok(MaybeEmptyIter::default()),
        };
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
        }
        .into())
    }
}
