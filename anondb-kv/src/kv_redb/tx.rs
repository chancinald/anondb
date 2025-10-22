use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Result;

use redb::*;

use super::*;

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

pub struct RedbReadTransaction {
    pub read: redb::ReadTransaction,
    pub tables: RwLock<HashMap<String, Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>>,
}

pub struct RedbWriteTransaction {
    pub write: redb::WriteTransaction,
}

impl RedbReadTransaction {
    fn read_table(&self, name: &str) -> Result<Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>> {
        if let Some(table) = self.tables.read().unwrap().get(name) {
            Ok(table.clone())
        } else {
            let table = self
                .read
                .open_table(TableDefinition::<&[u8], &[u8]>::new(name))?;
            let table = Arc::new(table);
            self.tables
                .write()
                .unwrap()
                .insert(name.into(), table.clone());
            Ok(table)
        }
    }
}

impl WriteTx for RedbWriteTransaction {
    fn commit(self) -> Result<()> {
        self.write.commit()?;
        Ok(())
    }
}

impl WriteOperations for RedbWriteTransaction {
    fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        table.insert(key, value)?;
        Ok(())
    }

    fn remove_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        let removed = table.remove(key, value)?;
        Ok(removed)
    }

    fn remove_all_multimap(&self, table: &str, key: &[u8]) -> Result<()> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        table.remove_all(key)?;
        Ok(())
    }

    fn clear_multimap(&self, table: &str) -> Result<()> {
        self.write.delete_multimap_table(tabledef_multimap(table))?;
        Ok(())
    }

    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut table = self.write.open_table(tabledef(table))?;
        Ok(table.remove(key)?.map(|v| v.value().to_vec()))
    }

    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut table = self.write.open_table(tabledef(table))?;
        Ok(table.insert(key, value)?.map(|v| v.value().to_vec()))
    }

    fn clear(&self, table: &str) -> Result<()> {
        self.write.delete_table(tabledef(table))?;
        Ok(())
    }
}

impl ReadOperations for RedbReadTransaction {
    fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        let table = self.read.open_multimap_table(tabledef_multimap(table))?;
        let inner_iter = table.get(key)?;
        Ok(RedbReadIter {
            data: Arc::new(key.to_vec()),
            tx: self.into(),
            inner_iter,
            map_fn: |key, item| {
                let val = item?;
                Ok(RedbItem {
                    item: (key.into(), val.into()),
                })
            },
        })
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .read_table(table)?
            .get(key)?
            .map(|v| v.value().to_vec()))
    }

    fn count(&self, table: &str) -> Result<u64> {
        Ok(self.read_table(table)?.len()?)
    }

    fn range<'a>(
        &'a self,
        table: String,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let table = self.read.open_table(tabledef(&table))?;
        let inner_iter = table.range(range)?;
        Ok(RedbReadIter {
            data: Arc::new(()),
            tx: self.into(),
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
        let table = self.read.open_multimap_table(tabledef_multimap(&table))?;
        let inner_iter = FlatMapFallible::from(table.range(range)?.map(|v| {
            let (key, vals) = v?;
            let key = Arc::new(key);
            Ok(vals.map(move |v| Ok((key.clone(), v?))))
        }));
        Ok(RedbReadIter {
            data: Arc::new(()),
            tx: self.into(),
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

impl ReadOperations for RedbWriteTransaction {
    fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        panic!();
        Ok(Vec::<Result<RedbItem>>::default().into_iter())
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table.get(key)?.map(|v| v.value().to_vec()))
    }

    fn count(&self, table: &str) -> Result<u64> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table.len()?)
    }

    fn range<'a>(
        &'a self,
        table: String,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        panic!();
        Ok(Vec::<Result<RedbItem>>::default().into_iter())
    }

    fn range_multimap<'a>(
        &'a self,
        table: String,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        panic!();
        Ok(Vec::<Result<RedbItem>>::default().into_iter())
    }
}
