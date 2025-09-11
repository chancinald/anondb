use anyhow::Result;
use redb::*;

use crate::*;

pub struct JournaledTable<'tx> {
    table: redb::Table<'tx, Bytes, Bytes>,
    tx: &'tx JournaledTransaction<'tx>,
    journal: &'tx Journal,
}

impl<'tx> JournaledTable<'tx> {
    pub fn new(
        table: Table<'tx, Bytes, Bytes>,
        journal: &'tx Journal,
        tx: &'tx JournaledTransaction<'tx>,
    ) -> Self {
        Self { table, journal, tx }
    }

    pub fn len(&self) -> Result<u64> {
        Ok(self.table.len()?)
    }

    pub fn insert_bytes(
        &mut self,
        key_bytes: &Bytes,
        value_bytes: &Bytes,
    ) -> Result<Option<AccessGuard<Bytes>>> {
        let table_name = self.table.name().into();

        let out = self.table.insert(key_bytes, value_bytes)?;

        self.tx.operate(TransactionOperation::Insert {
            table_name,
            key_bytes: key_bytes.clone(),
            value_bytes: value_bytes.clone(),
        })?;

        Ok(out)
    }

    pub fn insert<K, V>(&mut self, key: &K, value: &V) -> Result<Option<AccessGuard<Bytes>>>
    where
        K: serde::Serialize,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        self.insert_bytes(&Bytes::encode(key)?, &Bytes::encode(value)?)
    }

    pub fn remove_bytes(&mut self, key_bytes: &Bytes) -> Result<Option<AccessGuard<Bytes>>> {
        let table_name = self.table.name().to_string();
        let out = self.table.remove(key_bytes)?;
        self.tx
            .operate(TransactionOperation::Remove(table_name, key_bytes.clone()))?;
        Ok(out)
    }

    pub fn remove<S>(&mut self, key: &S) -> Result<Option<AccessGuard<Bytes>>>
    where
        S: serde::Serialize,
    {
        self.remove_bytes(&Bytes::encode(key)?)
    }
}
