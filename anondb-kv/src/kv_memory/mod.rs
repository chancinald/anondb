use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::OwnedRwLockReadGuard;
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
    tables: Arc<RwLock<HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>>>,
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
            tables_lock: self.tables.clone().read_owned().await,
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        Ok(MemoryKVWriteTransaction::new(
            self.tables.clone().write_owned().await,
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
        let v = {
            let tx = self.read_tx().await?;
            tx.get(table, key).await?
        };
        Ok(v)
    }
}

impl WriteOperations for MemoryKV {
    type Item = MemoryKVItem;
    async fn clear(&self, table: &str) -> Result<()> {
        match self.tables.write().await.get_mut(table) {
            Some(table) => {
                table.clear();
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
        Ok(tables
            .entry(table.into())
            .or_default()
            .insert(key.into(), value.into()))
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.tables.write().await.get_mut(table) {
            Some(table) => Ok(table.remove(key)),
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(None)
            }
        }
    }
}
