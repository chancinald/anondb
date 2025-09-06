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

    pub fn insert_bytes(
        &mut self,
        key_bytes: Vec<u8>,
        value_bytes: Vec<u8>,
    ) -> Result<Option<AccessGuard<Bytes>>> {
        let table_name = self.table.name().into();

        let out = self
            .table
            .insert(key_bytes.as_slice(), value_bytes.as_slice())?;

        self.tx.operate(TransactionOperation::Insert {
            table_name,
            key_bytes,
            value_bytes,
        })?;

        Ok(out)
    }

    pub fn insert<K, V>(&mut self, key: &K, value: &V) -> Result<Option<AccessGuard<Bytes>>>
    where
        K: serde::Serialize + ?Sized,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let key_bytes = rmp_serde::to_vec(key)?;
        let value_bytes = rmp_serde::to_vec(value)?;
        self.insert_bytes(key_bytes, value_bytes)
    }

    pub fn remove_bytes(&mut self, key_bytes: Vec<u8>) -> Result<Option<AccessGuard<Bytes>>> {
        let table_name = self.table.name().to_string();
        let out = self.table.remove(key_bytes.as_slice())?;
        self.tx
            .operate(TransactionOperation::Remove(table_name, key_bytes))?;
        Ok(out)
    }

    pub fn remove<S>(&mut self, key: &S) -> Result<Option<AccessGuard<Bytes>>>
    where
        S: serde::Serialize,
    {
        let key_bytes = rmp_serde::to_vec(key)?;
        self.remove_bytes(key_bytes)
    }
}
