use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Result;

use redb::*;

use super::*;

pub struct RedbReadTransaction {
    pub read: redb::ReadTransaction,
    pub tables: RwLock<HashMap<String, Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>>,
    pub multimap_tables:
        RwLock<HashMap<String, Arc<ReadOnlyMultimapTable<&'static [u8], &'static [u8]>>>>,
}

pub struct RedbWriteTransaction {
    pub write: redb::WriteTransaction,
}

impl RedbReadTransaction {
    pub fn read_table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>> {
        if let Some(table) = self.tables.read().unwrap().get(name) {
            return Ok(Some(table.clone()));
        }
        let table = match self.read.open_table(tabledef(name)) {
            Ok(t) => t,
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    return Ok(None);
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
        let table = Arc::new(table);
        self.tables
            .write()
            .unwrap()
            .insert(name.into(), table.clone());
        Ok(Some(table))
    }

    pub fn read_multimap_table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<ReadOnlyMultimapTable<&'static [u8], &'static [u8]>>>> {
        if let Some(table) = self.multimap_tables.read().unwrap().get(name) {
            return Ok(Some(table.clone()));
        }
        let table = match self.read.open_multimap_table(tabledef_multimap(name)) {
            Ok(t) => t,
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    return Ok(None);
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
        let table = Arc::new(table);
        self.multimap_tables
            .write()
            .unwrap()
            .insert(name.into(), table.clone());
        Ok(Some(table))
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
        let table = match self.read_multimap_table(table)? {
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

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.read_table(table)? {
            Some(table) => Ok(table.get(key)?.map(|v| v.value().to_vec())),
            None => Ok(None),
        }
    }

    fn count_multimap(&self, table: &str) -> Result<u64> {
        match self.read_multimap_table(table)? {
            Some(table) => Ok(table.len()?),
            None => Ok(0),
        }
    }

    fn count(&self, table: &str) -> Result<u64> {
        match self.read_table(table)? {
            Some(table) => Ok(table.len()?),
            None => Ok(0),
        }
    }

    fn range<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]>,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        let table = match self.read_table(&table)? {
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
        let table = match self.read_multimap_table(&table)? {
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

impl ReadOperations for RedbWriteTransaction {
    fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        // this implementation allocates all items from the entry into memory
        // this is because of limitations with lifetimes in redb transactions
        let key = Arc::new(key.to_vec());
        let table = self.write.open_multimap_table(tabledef_multimap(table))?;
        let mut entry = table.get(key.as_slice())?;
        let mut out = Vec::default();
        while let Some(item) = entry.next() {
            let val = item?;
            out.push(Ok(RedbItem {
                item: (key.clone().into(), val.value().to_vec().into()),
            }));
        }
        Ok(out.into_iter())
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table.get(key)?.map(|v| v.value().to_vec()))
    }

    fn count_multimap(&self, table: &str) -> Result<u64> {
        let table = self.write.open_multimap_table(tabledef_multimap(table))?;
        Ok(table.len()?)
    }

    fn count(&self, table: &str) -> Result<u64> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table.len()?)
    }

    fn range<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        // this implementation allocates all items from the range into memory
        // this is because of limitations with lifetimes in redb transactions
        let table = self.write.open_table(tabledef(&table))?;
        let mut entry = table.range(range)?;
        let mut out = Vec::default();
        while let Some(item) = entry.next() {
            let (key, val) = item?;
            out.push(Ok(RedbItem {
                item: (key.value().to_vec().into(), val.value().to_vec().into()),
            }));
        }
        drop(table);
        Ok(out.into_iter())
    }

    fn range_multimap<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<&'a [u8]> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>> + 'a> {
        // this implementation allocates all items from the range into memory
        // this is because of limitations with lifetimes in redb transactions
        let table = self.write.open_multimap_table(tabledef_multimap(&table))?;
        let mut entry = table.range(range)?;
        let mut out = Vec::default();
        while let Some(item) = entry.next() {
            let (key, mut values) = item?;
            let key = Arc::new(key.value().to_vec());
            while let Some(val) = values.next() {
                let val = val?;
                out.push(Ok(RedbItem {
                    item: (key.clone().into(), val.value().to_vec().into()),
                }));
            }
        }
        Ok(out.into_iter())
    }
}
