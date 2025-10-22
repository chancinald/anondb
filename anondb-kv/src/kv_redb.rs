use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Result;

use redb::*;

use super::*;

pub struct RedbIterator<'a> {
    transaction_owned: Option<RedbTransaction>,
    transaction: Option<&'a RedbTransaction>,
    range: Option<Range<'a, &'static [u8], &'static [u8]>>,
}

impl<'a> From<&'a RedbTransaction> for RedbIterator<'a> {
    fn from(value: &'a RedbTransaction) -> Self {
        Self {
            transaction_owned: None,
            transaction: Some(value),
            range: None,
        }
    }
}

impl<'a> From<RedbTransaction> for RedbIterator<'a> {
    fn from(value: RedbTransaction) -> Self {
        Self {
            transaction_owned: Some(value),
            transaction: None,
            range: None,
        }
    }
}

impl<'a> RedbIterator<'a> {
    pub fn range(mut self, table: &str, range: impl RangeBounds<Vec<u8>>) -> Result<Self> {
        let tx = self
            .transaction
            .unwrap_or_else(|| self.transaction_owned.as_ref().unwrap());
        match tx {
            RedbTransaction::Read(_, _) => {
                let table = tx.read_table(table)?;
                let range_ref = KeyRange {
                    start: range.start_bound().map(|v| v.as_slice()),
                    end: range.end_bound().map(|v| v.as_slice()),
                };
                self.range = Some(table.range::<&[u8]>(range_ref)?);
            }
            RedbTransaction::Write { .. } => {
                unimplemented!()
            }
        }
        Ok(self)
    }
}

impl<'a> Iterator for RedbIterator<'a> {
    type Item = Result<RedbItem<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.range.is_none() {
            return None;
        }
        let range = self.range.as_mut().unwrap();
        if let Some(item) = range.next() {
            Some(
                item.map_err(|e| anyhow::anyhow!(e))
                    .map(|v| RedbItem { item: v }),
            )
        } else {
            None
        }
    }
}

pub struct RedbItem<'a> {
    item: (
        AccessGuard<'a, &'static [u8]>,
        AccessGuard<'a, &'static [u8]>,
    ),
}

impl<'a> OpaqueItem for RedbItem<'a> {
    fn key(&self) -> &[u8] {
        self.item.0.value()
    }

    fn value(&self) -> &[u8] {
        self.item.1.value()
    }
}

pub enum RedbTransaction {
    Read(
        ReadTransaction,
        RwLock<HashMap<String, Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>>,
    ),
    Write {
        write: WriteTransaction,
    },
}

impl RedbTransaction {
    fn read_table(&self, name: &str) -> Result<Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>> {
        match self {
            RedbTransaction::Read(read, tables) => {
                if let Some(table) = tables.read().unwrap().get(name) {
                    Ok(table.clone())
                } else {
                    let table = read.open_table(TableDefinition::<&[u8], &[u8]>::new(name))?;
                    let table = Arc::new(table);
                    tables.write().unwrap().insert(name.into(), table.clone());
                    Ok(table)
                }
            }
            RedbTransaction::Write { .. } => {
                anyhow::bail!("RedbKV: Attempting to open a write table in a read transaction!");
            }
        }
    }

    fn write_table<T, F>(&self, name: &str, op: F) -> Result<T>
    where
        F: FnOnce(Table<'_, &'static [u8], &'static [u8]>) -> Result<T>,
    {
        match self {
            RedbTransaction::Read(_, _) => {
                anyhow::bail!("RedbKV: Attempting to open a read table in a write transaction!");
            }
            RedbTransaction::Write { write, .. } => {
                let table = write.open_table(TableDefinition::<&[u8], &[u8]>::new(name))?;
                op(table)
            }
        }
    }
}

impl Transaction for RedbTransaction {
    fn commit(self) -> Result<()> {
        match self {
            RedbTransaction::Read(_, _) => {
                anyhow::bail!("RedbKV: Attempting to commit a read transaction!");
            }
            RedbTransaction::Write { write, .. } => {
                write.commit()?;
                Ok(())
            }
        }
    }
}

impl Operations for RedbTransaction {
    fn get_multimap(&self, _table: &str, _key: &[u8]) -> Result<impl Iterator<Item = &[u8]>> {
        Ok(vec![panic!()].into_iter())
    }

    fn insert_multimap(&self, _table: &str, _key: &[u8], _value: &[u8]) -> Result<()> {
        unimplemented!()
    }

    fn remove_multimap(&self, _table: &str, _key: &[u8], _value: &[u8]) -> Result<bool> {
        unimplemented!()
    }

    fn remove_all_multimap(&self, _table: &str, _key: &[u8]) -> Result<()> {
        unimplemented!()
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            RedbTransaction::Read(_, _) => Ok(self
                .read_table(table)?
                .get(key)?
                .map(|v| v.value().to_vec())),
            RedbTransaction::Write { .. } => self.write_table(table, |table| {
                Ok(table.get(key)?.map(|v| v.value().to_vec()))
            }),
        }
    }

    fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            RedbTransaction::Read(_, _) => {
                anyhow::bail!(
                    "RedbKV: Attempting to execute mutable operation (remove) in read-only transaction"
                );
            }
            RedbTransaction::Write { .. } => self.write_table(table, |mut table| {
                Ok(table.remove(key)?.map(|v| v.value().to_vec()))
            }),
        }
    }

    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            RedbTransaction::Read(_, _) => {
                anyhow::bail!(
                    "RedbKV: Attempting to execute mutable operation (insert) in read-only transaction"
                );
            }
            RedbTransaction::Write { .. } => self.write_table(table, |mut table| {
                Ok(table.insert(key, value)?.map(|v| v.value().to_vec()))
            }),
        }
    }

    fn count(&self, table: &str) -> Result<u64> {
        match self {
            RedbTransaction::Read(_, _) => Ok(self.read_table(table)?.len()?),
            RedbTransaction::Write { .. } => self.write_table(table, |table| Ok(table.len()?)),
        }
    }

    fn clear(&self, table: &str) -> Result<()> {
        match self {
            RedbTransaction::Read(_, _) => {
                anyhow::bail!(
                    "RedbKV: Attempting to execute mutable operation (clear) in read-only transaction"
                );
            }
            RedbTransaction::Write { write, .. } => {
                write.delete_table(TableDefinition::<&[u8], &[u8]>::new(table))?;
                Ok(())
            }
        }
    }

    fn range<'a>(
        &self,
        table: String,
        is_multimap: bool,
        range: impl RangeBounds<Vec<u8>> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        let iterator = RedbIterator::from(self);
        iterator.range(&table, range)
    }

    // fn range_multimap<'a, T: for<'de> Deserialize<'de>>(
    //     &self,
    //     table: String,
    //     range: impl RangeBounds<Vec<u8>> + 'a,
    //     dir: SortDirection,
    //     handler: impl Fn(&[u8], &[u8]) -> Result<(Option<T>, bool)>,
    // ) -> Result<impl Iterator<Item = T>> {
    //     let mut out = Vec::default();
    //     let range_ref = KeyRange {
    //         start: range.start_bound().map(|v| v.as_slice()),
    //         end: range.end_bound().map(|v| v.as_slice()),
    //     };
    //     match self {
    //         RedbTransaction::Read(read, _) => {
    //             let table =
    //                 read.open_multimap_table(MultimapTableDefinition::<&[u8], &[u8]>::new(&table))?;
    //             let mut range = table.range::<&[u8]>(range_ref)?;
    //             while let Some(item) = match dir {
    //                 SortDirection::Asc => range.next(),
    //                 SortDirection::Desc => range.next_back(),
    //             } {
    //                 let (key, vals) = item?;
    //                 let key = key.value();
    //                 for val in vals {
    //                     let val = val?;
    //                     let (item_maybe, cont) = handler(key, val.value())?;
    //                     if let Some(item) = item_maybe {
    //                         out.push(item);
    //                     }
    //                     if !cont {
    //                         return Ok(out.into_iter());
    //                     }
    //                 }
    //             }
    //         }
    //         RedbTransaction::Write { write, .. } => {
    //             let table = write
    //                 .open_multimap_table(MultimapTableDefinition::<&[u8], &[u8]>::new(&table))?;
    //             let mut range = table.range::<&[u8]>(range_ref)?;
    //             while let Some(item) = match dir {
    //                 SortDirection::Asc => range.next(),
    //                 SortDirection::Desc => range.next_back(),
    //             } {
    //                 let (key, vals) = item?;
    //                 let key = key.value();
    //                 for val in vals {
    //                     let val = val?;
    //                     let (item_maybe, cont) = handler(key, val.value())?;
    //                     if let Some(item) = item_maybe {
    //                         out.push(item);
    //                     }
    //                     if !cont {
    //                         return Ok(out.into_iter());
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     Ok(out.into_iter())
    // }
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
    type Transaction = RedbTransaction;

    fn at_path(path: &std::path::Path) -> Result<Self> {
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

    /// Iterate over the contents of a collection, in ascending lexicographic order. Must be
    /// `O(N)`.
    fn scan<S>(&self, table: &str, predicate: S) -> Result<()>
    where
        S: Fn(&[u8], &[u8]) -> Result<bool>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<&[u8], &[u8]>::new(table));
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(());
            }
        }
        let table = table?;
        let mut range = table.range::<&[u8]>(..)?;
        while let Some(item) = range.next() {
            let item = item?;
            if !predicate(item.0.value(), item.1.value())? {
                break;
            }
        }
        Ok(())
    }

    fn write_tx(&self) -> Result<Self::Transaction> {
        Ok(RedbTransaction::Write {
            write: self.db.begin_write()?,
        })
    }

    fn read_tx(&self) -> Result<Self::Transaction> {
        let read = self.db.begin_read()?;
        Ok(RedbTransaction::Read(read, RwLock::new(HashMap::default())))
    }
}

/// Operations occuring outside of a transaction. "One and done" operations.
impl Operations for RedbKV {
    fn get_multimap(&self, _table: &str, _key: &[u8]) -> Result<impl Iterator<Item = &[u8]>> {
        Ok(vec![panic!()].into_iter())
    }

    fn insert_multimap(&self, _table: &str, _key: &[u8], _value: &[u8]) -> Result<()> {
        unimplemented!()
    }

    fn remove_multimap(&self, _table: &str, _key: &[u8], _value: &[u8]) -> Result<bool> {
        unimplemented!()
    }

    fn remove_all_multimap(&self, _table: &str, _key: &[u8]) -> Result<()> {
        unimplemented!()
    }

    /// Insert a key for a table and return the old value if it exists. Must be `O(1)`.
    fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx = self.db.begin_write()?;
        let mut table = tx.open_table(TableDefinition::<&[u8], &[u8]>::new(table))?;

        let out = if let Some(old_val) = table.insert(key, value)? {
            Some(old_val.value().to_vec())
        } else {
            None
        };

        drop(table);
        tx.commit()?;

        Ok(out)
    }

    /// Remove a key from a table. Must be `O(1)`.
    fn remove(&self, _table: &str, _key: &[u8]) -> Result<Option<Vec<u8>>> {
        unimplemented!()
    }

    fn clear(&self, table: &str) -> Result<()> {
        let tx = self.db.begin_write()?;
        tx.delete_table(TableDefinition::<&[u8], &[u8]>::new(table))?;
        tx.commit()?;
        Ok(())
    }

    fn count(&self, table: &str) -> Result<u64> {
        let read = self.db.begin_read()?;
        match read.open_table(TableDefinition::<&[u8], &[u8]>::new(table)) {
            Ok(table) => Ok(table.len()?),
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Retrieve the value associated to a key for a table. Must be `O(1)`.
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<&[u8], &[u8]>::new(table));
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(None);
            }
        }
        let table = table?;
        if let Some(val) = table.get(key)? {
            Ok(Some(val.value().to_vec()))
        } else {
            Ok(None)
        }
    }

    // TODO: custom iterator implementation to avoid buffering into a vec
    fn range<'a>(
        &self,
        table: String,
        is_multimap: bool,
        range: impl RangeBounds<Vec<u8>> + 'a,
    ) -> Result<impl Iterator<Item = Result<impl OpaqueItem>>> {
        let tx = self.read_tx()?;
        let iterator = RedbIterator::from(tx);
        iterator.range(&table, range)
    }
}
