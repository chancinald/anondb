use rpds::RedBlackTreeMapSync;
use tokio::sync::Mutex;

use super::*;

pub struct MemoryKVWriteTransaction {
    tables_snapshot: Mutex<(
        HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
        HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
    )>,
    pub(crate) tables_lock:
        OwnedRwLockWriteGuard<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>,
    pub(crate) tables_multi_lock:
        OwnedRwLockWriteGuard<HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>>,
    /// The snapshot readers use while the tables are locked; updated on commit.
    shared_snapshot: Arc<
        RwLock<(
            HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
            HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
        )>,
    >,
}

impl MemoryKVWriteTransaction {
    pub fn new(
        tables_lock: OwnedRwLockWriteGuard<HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>>,
        tables_multi_lock: OwnedRwLockWriteGuard<
            HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
        >,
        shared_snapshot: Arc<
            RwLock<(
                HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
                HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
            )>,
        >,
    ) -> Self {
        Self {
            tables_snapshot: Mutex::new((tables_lock.clone(), tables_multi_lock.clone())),
            tables_lock,
            tables_multi_lock,
            shared_snapshot,
        }
    }
}

impl ReadOperations for MemoryKVWriteTransaction {
    type Item<'tx> = MemoryKVItem;
    async fn count(&self, table: &str) -> Result<u64> {
        match self.tables_snapshot.lock().await.0.get(table) {
            Some(table) => Ok(table.size() as u64),
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(0)
            }
        }
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<MemoryKVItem>> {
        if let Some(table) = self.tables_snapshot.lock().await.0.get(table)
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

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        Ok(self
            .tables_snapshot
            .lock()
            .await
            .1
            .get(table)
            .map(|v| v.size() as u64)
            .unwrap_or_default())
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        Ok(self
            .tables_snapshot
            .lock()
            .await
            .1
            .get(table)
            .and_then(|v| v.get(key))
            .map(|v| v.len() as u64)
            .unwrap_or_default())
    }
}

impl WriteOperations for MemoryKVWriteTransaction {
    type Item<'tx> = MemoryKVItem;
    async fn clear(&self, table: &str) -> Result<()> {
        if self.tables_snapshot.lock().await.0.remove(table).is_none() {
            log::warn!("table \"{table}\" not found");
        }
        Ok(())
    }

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut snapshot = self.tables_snapshot.lock().await;
        let table = snapshot.0.entry(table.into()).or_default();
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
        match self.tables_snapshot.lock().await.0.get_mut(table) {
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
        let mut tables = self.tables_snapshot.lock().await;
        if let Some(table_data) = tables.0.remove(table) {
            tables.0.insert(new_name.into(), table_data);
            Ok(())
        } else {
            anyhow::bail!("table \"{table}\" not found");
        }
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let mut snapshot = self.tables_snapshot.lock().await;
        let tables_multi = snapshot.1.entry(table.into()).or_default();
        if let Some(table) = tables_multi.get_mut(key) {
            table.insert(value.to_vec());
        } else {
            let mut set = HashSet::new();
            set.insert(value.to_vec());
            tables_multi.insert_mut(key.to_vec(), set);
        }
        Ok(())
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let mut snapshot = self.tables_snapshot.lock().await;
        Ok(snapshot
            .1
            .get_mut(table)
            .and_then(|v| v.get_mut(key))
            .map(|v| v.remove(value))
            .unwrap_or_default())
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        let mut snapshot = self.tables_snapshot.lock().await;
        Ok(snapshot
            .1
            .get_mut(table)
            .map(|v| v.remove_mut(key))
            .unwrap_or_default())
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        if self.tables_snapshot.lock().await.1.remove(table).is_none() {
            log::warn!("multimap table \"{table}\" not found");
        }
        Ok(())
    }
}

impl WriteTx for MemoryKVWriteTransaction {
    type Item<'tx> = MemoryKVItem;
    async fn commit(mut self) -> Result<()> {
        let snapshot = self.tables_snapshot.into_inner();
        // Readers fall back to the shared snapshot while the tables are locked, which
        // includes the window where the next writer has locked them but not yet refreshed
        // it. Update it before releasing the tables so committed data is always visible.
        *self.shared_snapshot.write().await = (snapshot.0.clone(), snapshot.1.clone());
        *self.tables_lock = snapshot.0;
        *self.tables_multi_lock = snapshot.1;
        Ok(())
    }
}
