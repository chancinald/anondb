mod iter;
mod tx;

use iter::*;
use tx::*;

use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

use anyhow::Result;

use redb::*;

use super::*;

static EMPTY_BYTES: LazyLock<Vec<u8>> = LazyLock::new(|| vec![]);

fn tabledef(name: &str) -> TableDefinition<&'static [u8], &'static [u8]> {
    TableDefinition::new(name)
}

fn tabledef_multimap(name: &str) -> MultimapTableDefinition<&'static [u8], &'static [u8]> {
    MultimapTableDefinition::new(name)
}

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

pub struct RedbItem<'a> {
    item: (MaybeGuarded<'a>, MaybeGuarded<'a>),
}

impl<'a> OpaqueItem for RedbItem<'a> {
    fn key(&self) -> &[u8] {
        self.item.0.value()
    }

    fn value(&self) -> &[u8] {
        self.item.1.value()
    }
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
            tx: tx.into(),
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
            tx: tx.into(),
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
            tx: tx.into(),
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
