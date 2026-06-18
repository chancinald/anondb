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

#[derive(Clone, Debug)]
pub struct MemStorageBackend {
    data: Arc<RwLock<Vec<u8>>>,
}

impl MemStorageBackend {
    pub fn as_bytes(&self) -> Vec<u8> {
        self.data.read().unwrap().iter().copied().collect()
    }
}

impl From<Option<&[u8]>> for MemStorageBackend {
    fn from(value: Option<&[u8]>) -> Self {
        let data = if let Some(data) = value {
            data.to_vec()
        } else {
            vec![]
        };
        Self {
            data: Arc::new(RwLock::new(data)),
        }
    }
}

impl StorageBackend for MemStorageBackend {
    fn len(&self) -> std::result::Result<u64, std::io::Error> {
        Ok(self
            .data
            .read()
            .unwrap()
            .len()
            .try_into()
            .expect("storage len cast failed"))
    }
    fn read(&self, offset: u64, len: usize) -> std::result::Result<Vec<u8>, std::io::Error> {
        let offset: usize = offset.try_into().unwrap();
        let data = self.data.read().unwrap();
        Ok((0..len).map(|i| data[offset + i]).collect())
    }
    fn write(&self, offset: u64, new_data: &[u8]) -> std::result::Result<(), std::io::Error> {
        let offset: usize = offset.try_into().unwrap();
        let mut data = self.data.write().unwrap();
        for (i, v) in new_data.iter().enumerate() {
            data[offset + i] = *v;
        }
        Ok(())
    }
    fn set_len(&self, len: u64) -> std::result::Result<(), std::io::Error> {
        let len: usize = len.try_into().unwrap();
        self.data.write().unwrap().resize(len, 0u8);
        Ok(())
    }
    fn sync_data(&self, _eventual: bool) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct RedbKV {
    db: Database,
    backend: MemStorageBackend,
}

impl KV for RedbKV {
    type ReadTransaction = RedbReadTransaction;
    type WriteTransaction = RedbWriteTransaction;

    fn at_path(_path: &std::path::Path) -> Result<Self> {
        unimplemented!()
    }

    fn as_bytes(&self) -> Result<impl Iterator<Item = u8>> {
        Ok(self.backend.as_bytes().into_iter())
    }

    fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self> {
        let backend = MemStorageBackend::from(bytes_maybe);
        Ok(Self {
            db: Database::builder().create_with_backend(backend.clone())?,
            backend,
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
