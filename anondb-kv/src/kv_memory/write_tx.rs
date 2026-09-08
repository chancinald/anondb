use super::*;

pub enum WriteOperation {
    Remove(String, Vec<u8>),
    Insert(String, Vec<u8>, Vec<u8>),
    Clear(String),
}

pub struct MemoryKVWriteTransaction {
    pub(crate) tables_lock: OwnedRwLockWriteGuard<HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>>,
    pending_operations: Arc<RwLock<Vec<WriteOperation>>>,
}

impl MemoryKVWriteTransaction {
    pub fn new(
        tables_lock: OwnedRwLockWriteGuard<HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>>,
    ) -> Self {
        Self {
            tables_lock,
            pending_operations: Arc::new(RwLock::new(Default::default())),
        }
    }
}

impl ReadOperations for MemoryKVWriteTransaction {
    type Item = MemoryKVItem;
    async fn count(&self, table: &str) -> Result<u64> {
        match self.tables_lock.get(table) {
            Some(table) => Ok(table.len() as u64),
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(0)
            }
        }
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<MemoryKVItem>> {
        if let Some(table) = self.tables_lock.get(table)
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
    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        self.pending_operations
            .write()
            .await
            .push(WriteOperation::Insert(
                table.into(),
                key.to_vec(),
                value.to_vec(),
            ));
        Ok(None)
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.pending_operations
            .write()
            .await
            .push(WriteOperation::Remove(table.into(), key.to_vec()));
        Ok(None)
    }

    async fn clear(&self, table: &str) -> Result<()> {
        self.pending_operations
            .write()
            .await
            .push(WriteOperation::Clear(table.into()));
        Ok(())
    }
}

impl WriteTx for MemoryKVWriteTransaction {
    type Item = MemoryKVItem;
    async fn commit(mut self) -> Result<()> {
        for operation in self.pending_operations.write().await.drain(..) {
            match operation {
                WriteOperation::Insert(table, key, value) => {
                    self.tables_lock
                        .entry(table.into())
                        .or_default()
                        .insert(key.into(), value.into());
                }
                WriteOperation::Remove(table, key) => match self.tables_lock.get_mut(&table) {
                    Some(table) => {
                        table.remove(&key);
                    }
                    None => {
                        log::warn!("table \"{table}\" not found");
                    }
                },
                WriteOperation::Clear(table) => match self.tables_lock.get_mut(&table) {
                    Some(table) => {
                        table.clear();
                    }
                    None => {
                        log::warn!("table \"{table}\" not found");
                    }
                },
            }
        }
        Ok(())
    }
}
