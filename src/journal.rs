use std::collections::HashMap;

use anyhow::Result;
use redb::Database;
use redb::StorageBackend;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Serialize, Deserialize, Default)]
pub struct JournalState {
    pub next_tx_index: u64,
}

/// A journal structure for a single redb database. Replaying transactions from an empty database
/// should always yield equivalent databases.
///
/// Journal state is stored inside the redb instance and can be arbitrarily structured. A higher
/// level, statically analyzable, query system could be built as an abstraction around this.
pub struct Journal {
    pub db: Database,
}

impl From<Database> for Journal {
    fn from(value: Database) -> Self {
        Self { db: value }
    }
}

impl Journal {
    pub fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self> {
        let mem_backend = redb::backends::InMemoryBackend::new();
        if let Some(bytes) = bytes_maybe {
            mem_backend.write(0, bytes)?;
        }
        let db = redb::Database::builder().create_with_backend(mem_backend)?;
        Ok(Self::from(db))
    }

    /// Apply the operations and increment the transaction index.
    pub fn append_tx(&self, operations: &[TransactionOperation]) -> Result<()> {
        assert!(!operations.is_empty());
        let last_operation = operations.last().unwrap();
        assert_eq!(
            last_operation,
            &TransactionOperation::Commit,
            "final operation was not commit"
        );

        let mut tx = self.begin_write()?;

        {
            let mut tables_by_name: HashMap<String, JournaledTable> = HashMap::new();
            let mut multimap_tables_by_name = HashMap::new();
            for operation in &operations[..(operations.len() - 1)] {
                match operation {
                    TransactionOperation::OpenTable(table_name) => {
                        if !tables_by_name.contains_key(table_name) {
                            tables_by_name.insert(table_name.clone(), tx.open_table(table_name)?);
                        }
                    }
                    TransactionOperation::OpenMultimapTable(table_name) => {
                        if !multimap_tables_by_name.contains_key(&table_name) {
                            multimap_tables_by_name
                                .insert(table_name, tx.open_multimap_table(table_name)?);
                        }
                    }
                    TransactionOperation::Insert {
                        table_name,
                        key_bytes,
                        value_bytes,
                    } => {
                        if let Some(table) = tables_by_name.get_mut(table_name) {
                            table.insert_bytes(key_bytes, value_bytes)?;
                        } else {
                            anyhow::bail!("table {table_name} is not open");
                        }
                    }
                    TransactionOperation::Remove(table_name, key_bytes) => {
                        if let Some(table) = tables_by_name.get_mut(table_name) {
                            table.remove_bytes(key_bytes.into())?;
                        } else {
                            anyhow::bail!("table {table_name} is not open");
                        }
                    }
                    TransactionOperation::RenameTable(old_table_name, new_table_name) => {
                        tables_by_name.remove(old_table_name);
                        tx.rename_table(old_table_name, new_table_name)?;
                    }
                    TransactionOperation::RenameMultimapTable(old_table_name, new_table_name) => {
                        multimap_tables_by_name.remove(old_table_name);
                        tx.rename_multimap_table(old_table_name, new_table_name)?;
                    }
                    TransactionOperation::DeleteTable(table_name) => {
                        tables_by_name.remove(table_name);
                        tx.delete_table(table_name)?;
                    }
                    TransactionOperation::DeleteMultimapTable(table_name) => {
                        tx.delete_multimap_table(table_name)?;
                    }
                    TransactionOperation::Commit => {
                        anyhow::bail!("commit operation must be final operation");
                    }
                }
            }
        }

        tx.commit()?;

        Ok(())
    }

    pub fn get_state(&self) -> Result<JournalState> {
        // attempt to load the journal state
        let read = self.db.begin_read()?;
        // open the table if it exists, read the state key if it exists
        if let Ok(state_table) =
            read.open_table(TableDefinition::<Bytes, Bytes>::new(JOURNAL_TABLE))
        {
            if let Some(bytes) = state_table
                .get(Bytes::from(JOURNAL_STATE_KEY))
                .expect("failed to read open journal state table")
            {
                Ok(bytes.value().parse::<JournalState>()?)
            } else {
                Ok(JournalState::default())
            }
        } else {
            Ok(JournalState::default())
        }
    }

    pub fn begin_write(&self) -> Result<JournaledTransaction> {
        JournaledTransaction::new(self)
    }

    pub fn insert<K, V>(
        &self,
        table_name: &str,
        key: &K,
        value: &V,
    ) -> Result<(Option<V>, Vec<TransactionOperation>)>
    where
        K: serde::Serialize,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let mut tx = self.begin_write()?;
        let mut table = tx.open_table(table_name)?;

        let out = if let Some(old_val) = table.insert(key, value)? {
            Some(old_val.value().parse()?)
        } else {
            None
        };

        drop(table);

        Ok((out, tx.commit()?))
    }

    pub fn find_one<K, V, S>(&self, table_name: &str, selector: S) -> Result<Option<(K, V)>>
    where
        K: serde::Serialize + for<'de> serde::Deserialize<'de>,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
        S: Fn(K, V) -> Option<(K, V)>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name))?;
        let mut range = table.range::<Bytes>(..)?;
        while let Some(item) = range.next() {
            let item = item?;
            let key = item.0.value().parse()?;
            let value = item.1.value().parse()?;
            let out = selector(key, value);
            if out.is_some() {
                return Ok(out);
            }
        }
        Ok(None)
    }

    pub fn find_many<'a, K, V, S>(&self, table_name: &str, selector: S) -> Result<Vec<(K, V)>>
    where
        K: serde::Serialize + for<'de> serde::Deserialize<'de>,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
        S: Fn(&K, &V) -> bool,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name))?;
        let mut range = table.range::<Bytes>(..)?;
        let mut out = Vec::new();
        while let Some(item) = range.next() {
            let item = item?;
            let key = item.0.value().parse::<K>()?;
            let value = item.1.value().parse::<V>()?;
            if selector(&key, &value) {
                out.push((key, value));
            }
        }
        Ok(out)
    }

    pub fn get<K, V>(&self, table_name: &str, key: &K) -> Result<Option<V>>
    where
        K: serde::Serialize,
        V: for<'de> serde::Deserialize<'de>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name))?;
        if let Some(val) = table.get(Bytes::encode(key)?)? {
            Ok(val.value().parse()?)
        } else {
            Ok(None)
        }
    }
}
