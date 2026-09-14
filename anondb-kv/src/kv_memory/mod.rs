use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use rpds::RedBlackTreeMapSync;
use tokio::sync::OwnedRwLockWriteGuard;
use tokio::sync::RwLock;

use super::*;

mod read_tx;
mod write_tx;

use read_tx::*;
use write_tx::*;

#[derive(Debug)]
pub struct MemoryKVItem {
    pub(crate) key: Vec<u8>,
    pub(crate) value: Vec<u8>,
}

impl OpaqueItem for MemoryKVItem {
    fn key(&self) -> &[u8] {
        &self.key
    }

    async fn value(&self) -> &[u8] {
        &self.value
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemoryKV {
    /// A snapshot for read operations to access during write operations
    tables_snapshot: Arc<
        RwLock<(
            HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
            HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
        )>,
    >,
    /// The canonical KV data. This lock is distinct from the snapshot.
    tables: Arc<RwLock<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>>,
    /// Multimap tables
    tables_multi: Arc<RwLock<HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>>>,
}

impl MemoryKV {
    async fn get_read_snapshot(
        &self,
    ) -> (
        HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
        HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
    ) {
        if let Ok(tables) = self.tables.try_read()
            && let Ok(tables_multi) = self.tables_multi.try_read()
        {
            *self.tables_snapshot.write().await = (tables.clone(), tables_multi.clone());
            (tables.clone(), tables_multi.clone())
        } else {
            self.tables_snapshot.read().await.clone()
        }
    }
}

impl KV for MemoryKV {
    type Item = MemoryKVItem;
    type ReadTransaction = MemoryKVReadTransaction;
    type WriteTransaction = MemoryKVWriteTransaction;
    type Config = ();

    async fn new(_config_maybe: Option<Self::Config>) -> Result<Self> {
        Ok(Self::default())
    }

    async fn read_tx(&self) -> Result<Self::ReadTransaction> {
        let snapshot = self.get_read_snapshot().await;
        Ok(MemoryKVReadTransaction {
            tables: snapshot.0,
            tables_multi: snapshot.1,
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        let tables_lock = self.tables.clone().write_owned().await;
        let tables_multi_lock = self.tables_multi.clone().write_owned().await;
        *self.tables_snapshot.write().await = (tables_lock.clone(), tables_multi_lock.clone());
        Ok(MemoryKVWriteTransaction::new(
            tables_lock,
            tables_multi_lock,
        ))
    }
}

impl ReadOperations for MemoryKV {
    type Item = MemoryKVItem;
    async fn count(&self, table: &str) -> Result<u64> {
        let tx = self.read_tx().await?;
        Ok(tx.count(table).await?)
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<MemoryKVItem>> {
        self.read_tx().await?.get(table, key).await
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        self.read_tx().await?.count_multimap(table).await
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        self.read_tx()
            .await?
            .count_multimap_values(table, key)
            .await
    }
}

impl WriteOperations for MemoryKV {
    type Item = MemoryKVItem;
    async fn clear(&self, table: &str) -> Result<()> {
        match self.tables.write().await.get_mut(table) {
            Some(table) => {
                *table = RedBlackTreeMapSync::default();
                Ok(())
            }
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(())
            }
        }
    }

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut tables = self.tables.write().await;
        let table = tables.entry(table.into()).or_default();
        Ok(if let Some(val) = table.get_mut(key) {
            let mut out = value.to_vec();
            std::mem::swap(val, &mut out);
            Some(out)
        } else {
            table.insert_mut(key.into(), value.into());
            None
        })
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.tables.write().await.get_mut(table) {
            Some(table) => {
                let prev = table.get(key).cloned();
                table.remove_mut(key);
                Ok(prev)
            }
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(None)
            }
        }
    }

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        let mut tables = self.tables.write().await;
        if let Some(table_data) = tables.remove(table) {
            tables.insert(new_name.into(), table_data);
            Ok(())
        } else {
            anyhow::bail!("table \"{table}\" not found");
        }
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let mut tables_multi = self.tables_multi.write().await;
        let tables = tables_multi.entry(table.to_string()).or_default();
        if let Some(table) = tables.get_mut(key) {
            table.insert(value.to_vec());
        } else {
            let mut set = HashSet::new();
            set.insert(value.to_vec());
            tables.insert_mut(key.to_vec(), set);
        }
        Ok(())
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let tx = self.write_tx().await?;
        let out = tx.remove_multimap_value(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        let tx = self.write_tx().await?;
        let out = tx.remove_multimap(table, key).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        let tx = self.write_tx().await?;
        tx.clear_multimap(table).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl RangeReadOperations for MemoryKV {
    type Item = MemoryKVItem;
    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(async_stream::try_stream! {
            let tx = self.read_tx().await?;
            let out = tx.range(table, range).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(doc) = stream.next().await.transpose()? {
                yield doc;
            }
        })
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(async_stream::try_stream! {
            let tx = self.read_tx().await?;
            let out = tx.range_multimap(table, range).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(doc) = stream.next().await.transpose()? {
                yield doc;
            }
        })
    }

    async fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(async_stream::try_stream! {
            let tx = self.read_tx().await?;
            let out = tx.get_multimap(table, key).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(doc) = stream.next().await.transpose()? {
                yield doc;
            }
        })
    }
}
