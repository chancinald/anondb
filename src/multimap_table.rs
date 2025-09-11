use anyhow::Result;
use redb::*;

use crate::*;

pub struct JournaledMultimapTable<'tx> {
    table: redb::MultimapTable<'tx, Bytes, Bytes>,
    tx: &'tx ActiveTransaction<'tx>,
    journal: &'tx Journal,
}

impl<'tx> JournaledMultimapTable<'tx> {
    pub fn new(
        table: MultimapTable<'tx, Bytes, Bytes>,
        journal: &'tx Journal,
        tx: &'tx ActiveTransaction<'tx>,
    ) -> Self {
        Self { table, tx, journal }
    }

    /// Return the number of keys in the table.
    pub fn len(&self) -> Result<u64> {
        Ok(self.table.len()?)
    }

    pub fn insert_bytes(&mut self, key_bytes: &Bytes, value_bytes: &Bytes) -> Result<bool> {
        let table_name = self.table.name().into();

        let out = self.table.insert(key_bytes, value_bytes)?;

        self.tx.operate(TransactionOperation::Insert {
            table_name,
            key_bytes: key_bytes.clone(),
            value_bytes: value_bytes.clone(),
        })?;

        Ok(out)
    }

    /// Add a value to the key. Returns `true` if the value was present.
    pub fn insert<K, V>(&mut self, key: &K, value: &V) -> Result<bool>
    where
        K: serde::Serialize,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        self.insert_bytes(&Bytes::encode(key)?, &Bytes::encode(value)?)
    }
}
