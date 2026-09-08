use std::collections::HashMap;
use std::sync::Arc;

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
    key: Vec<u8>,
    value: Vec<u8>,
}

impl OpaqueItem for MemoryKVItem {
    fn key(&self) -> &[u8] {
        &self.key
    }

    async fn value(&self) -> &[u8] {
        &self.value
    }
}

#[derive(Default, Clone)]
pub struct MemoryKV {
    /// A snapshot for read operations to access during write operations
    tables_snapshot: Arc<RwLock<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>>,
    /// The canonical KV data. This lock is distinct from the snapshot.
    tables: Arc<RwLock<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>>,
}

impl MemoryKV {
    async fn get_read_snapshot(&self) -> HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>> {
        if let Ok(tables) = self.tables.try_read() {
            *self.tables_snapshot.write().await = tables.clone();
            tables.clone()
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
        Ok(MemoryKVReadTransaction {
            tables: self.get_read_snapshot().await,
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        let tables_lock = self.tables.clone().write_owned().await;
        *self.tables_snapshot.write().await = tables_lock.clone();
        Ok(MemoryKVWriteTransaction::new(tables_lock))
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
}
