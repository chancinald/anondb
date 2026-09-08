use rpds::RedBlackTreeMapSync;
use tokio::sync::Mutex;

use super::*;

pub struct MemoryKVWriteTransaction {
    pub(crate) tables_lock:
        Mutex<OwnedRwLockWriteGuard<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>>,
}

impl MemoryKVWriteTransaction {
    pub fn new(
        tables_lock: OwnedRwLockWriteGuard<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>,
    ) -> Self {
        Self {
            tables_lock: Mutex::new(tables_lock),
        }
    }
}

impl ReadOperations for MemoryKVWriteTransaction {
    type Item = MemoryKVItem;
    async fn count(&self, table: &str) -> Result<u64> {
        match self.tables_lock.lock().await.get(table) {
            Some(table) => Ok(table.size() as u64),
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(0)
            }
        }
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<MemoryKVItem>> {
        if let Some(table) = self.tables_lock.lock().await.get(table)
            && let Some(item) = table.get(key)
        {
            Ok(Some(MemoryKVItem {
                key: key.to_vec(),
                value: item.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

impl WriteOperations for MemoryKVWriteTransaction {
    type Item = MemoryKVItem;
    async fn clear(&self, table: &str) -> Result<()> {
        match self.tables_lock.lock().await.get_mut(table) {
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
        let mut tables = self.tables_lock.lock().await;
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
        match self.tables_lock.lock().await.get_mut(table) {
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

impl WriteTx for MemoryKVWriteTransaction {
    type Item = MemoryKVItem;
    async fn commit(self) -> Result<()> {
        Ok(())
    }
}
