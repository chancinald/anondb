use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use redb::*;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Serialize, Deserialize, Default)]
pub struct JournalState {
    pub next_tx_index: u64,
    pub last_tx_hash: [u8; 32],
}

/// A journal structure for a single redb database. Replaying transactions from an empty database
/// should always yield equivalent databases.
///
/// Journal state is stored inside the redb instance and can be arbitrarily structured. A higher
/// level, statically analyzable, query system could be built as an abstraction around this.
#[derive(Clone)]
pub struct Journal {
    pub db: Arc<Database>,
    transactions: (
        flume::Sender<JournalTransaction>,
        flume::Receiver<JournalTransaction>,
    ),
}

impl From<Database> for Journal {
    fn from(value: Database) -> Self {
        Self {
            db: Arc::new(value),
            transactions: flume::unbounded(),
        }
    }
}

impl Journal {
    // TODO: just make this a prefix
    pub fn system_tables() -> Vec<String> {
        vec![JOURNAL_TABLE_NAME.to_string(), TX_TABLE_NAME.to_string()]
    }

    pub fn flatten_at_index(&self, index: u64) -> Result<JournalTransaction> {
        self.at_index(index)?.flatten()
    }

    pub fn at_index(&self, index: u64) -> Result<Self> {
        let out = Self::in_memory(None)?;
        for i in 0..=index {
            if let Some(tx) = self.journal_tx_by_index(i)? {
                out.append_tx(&tx)?;
            } else {
                anyhow::bail!("No transaction for index {i}");
            }
        }
        Ok(out)
    }

    /// Flatten an entire database into a single journal transaction. This operation loads the
    /// entire database into memory. Database journal entries are not preserved, this produces a
    /// snapshot at the current index of the database.
    pub fn flatten(&self) -> Result<JournalTransaction> {
        let mut tx = JournalTransaction {
            last_tx_hash: <[u8; 32]>::default(),
            operations: Vec::default(),
        };
        let system_tables = Self::system_tables();
        let read = self.db.begin_read()?;
        // TODO: add multimap tables
        let tables = read.list_tables()?;
        for table in tables {
            let table_name = table.name().to_string();
            if system_tables.contains(&table_name) {
                continue;
            }
            let table = read.open_table(Self::table_definition(&table_name))?;
            tx.operations
                .push(TransactionOperation::OpenTable(table_name.clone()));
            let range = table.range::<Bytes>(..)?;
            for entry in range {
                let (key, val) = entry?;
                tx.operations.push(TransactionOperation::Insert {
                    table_name: table_name.clone(),
                    key_bytes: key.value(),
                    value_bytes: val.value(),
                });
            }
        }
        tx.operations.push(TransactionOperation::Commit);
        Ok(tx)
    }
    pub fn register_transaction(&self, tx: JournalTransaction) -> Result<()> {
        self.transactions.0.send(tx)?;
        Ok(())
    }

    pub fn drain_transactions(&self) -> Result<Vec<JournalTransaction>> {
        Ok(self.transactions.1.drain().collect())
    }

    pub fn table_definition(name: &str) -> TableDefinition<Bytes, Bytes> {
        TableDefinition::<Bytes, Bytes>::new(name)
    }

    pub fn multimap_table_definition(name: &str) -> MultimapTableDefinition<Bytes, Bytes> {
        MultimapTableDefinition::<Bytes, Bytes>::new(name)
    }

    pub fn at_path(path: &Path) -> Result<Self> {
        Ok(redb::Database::create(path)?.into())
    }

    pub fn in_memory(bytes_maybe: Option<&[u8]>) -> Result<Self> {
        let mem_backend = redb::backends::InMemoryBackend::new();
        if let Some(bytes) = bytes_maybe {
            mem_backend.write(0, bytes)?;
        }
        let db = redb::Database::builder().create_with_backend(mem_backend)?;
        Ok(Self::from(db))
    }

    /// Apply the operations, increment the transaction index, and persist the transaction in the
    /// database.
    pub fn append_tx(
        &self,
        JournalTransaction {
            operations,
            last_tx_hash,
        }: &JournalTransaction,
    ) -> Result<()> {
        let mut tx = self.begin_write()?;

        let state = self.get_state()?;
        if &state.last_tx_hash != last_tx_hash {
            anyhow::bail!("cannot apply transaction to divergent last_tx_hash");
        }

        if operations.is_empty() {
            anyhow::bail!("cannot apply empty transaction");
        }
        let last_operation = operations.last().unwrap();
        if last_operation != &TransactionOperation::Commit {
            anyhow::bail!("cannot apply transaction without final operation being commit");
        }

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
        // open the table if it exists, read the number of applied changes
        if let Ok(state_table) = read.open_table(JOURNAL_TABLE) {
            let next_tx_index = state_table.len()?;
            let last_tx_hash = if next_tx_index > 0 {
                match state_table.get(next_tx_index - 1)? {
                    Some(hash) => hash.value(),
                    None => anyhow::bail!("unable to find hash for latest tx index"),
                }
            } else {
                <[u8; 32]>::default()
            };
            Ok(JournalState {
                last_tx_hash,
                next_tx_index,
            })
        } else {
            Ok(JournalState::default())
        }
    }

    pub fn journal_tx_len(&self) -> Result<u64> {
        let read = self.db.begin_read()?;
        if let Ok(journal_table) = read.open_table(JOURNAL_TABLE) {
            Ok(journal_table.len()?)
        } else {
            Ok(0)
        }
    }

    pub fn journal_tx_by_index(&self, index: u64) -> Result<Option<JournalTransaction>> {
        let read = self.db.begin_read()?;
        if let Ok(journal_table) = read.open_table(JOURNAL_TABLE) {
            let tx_table = read.open_table(TX_TABLE)?;
            let tx_hash = journal_table.get(index)?;
            if tx_hash.is_none() {
                return Ok(None);
            }
            let tx_hash = tx_hash.unwrap();
            match tx_table.get(tx_hash.value())? {
                Some(tx_bytes) => Ok(Some(tx_bytes.value().parse::<JournalTransaction>()?)),
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Retrieve the current known journal transactions
    pub fn journal_transactions(&self) -> Result<Vec<JournalTransaction>> {
        let read = self.db.begin_read()?;
        if let Ok(journal_table) = read.open_table(JOURNAL_TABLE) {
            let tx_table = read.open_table(TX_TABLE)?;
            let mut txs = Vec::default();
            let mut range = journal_table.range::<u64>(..)?;
            while let Some(v) = range.next() {
                let (key, val) = v?;
                let tx_hash = val.value();
                let tx_index = key.value();
                match tx_table.get(tx_hash)? {
                    Some(tx) => {
                        let tx_bytes = tx.value();
                        txs.push(tx_bytes.parse()?);
                    }
                    None => anyhow::bail!(
                        "unable to find transaction data for index {}, hash {}",
                        tx_index,
                        hex::encode(tx_hash)
                    ),
                }
            }
            Ok(txs)
        } else {
            Ok(Vec::default())
        }
    }

    /// Wipe the database and replay up to `divergent_index - 1`. Then apply the canonical
    /// transactions. Attempt to apply the pending transactions after this.
    pub fn merge(
        &self,
        _divergent_index: u64,
        _canonical: Vec<JournalTransaction>,
        _pending: Vec<JournalTransaction>,
    ) -> Result<()> {
        // wipe db
        //
        // replay transactions
        //
        // apply canonical transactions
        //
        // for tx in pending
        //   check for conflicts between tx and canonical transactions?
        //   simply overwrite?
        //   user can always step backward to get previous versions of things
        unimplemented!()
    }

    pub fn begin_write(&self) -> Result<ActiveTransaction> {
        ActiveTransaction::new(self)
    }

    pub fn begin_read(&self) -> Result<ReadTransaction> {
        Ok(self.db.begin_read()?)
    }

    pub fn insert<K, V>(&self, table_name: &str, key: &K, value: &V) -> Result<Option<V>>
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
        tx.commit()?;

        Ok(out)
    }

    /// Scan a table for a specific key. May not be invoked on a multimap table.
    pub fn find_one<K, V, S>(&self, table_name: &str, selector: S) -> Result<Option<(K, V)>>
    where
        K: serde::Serialize + for<'de> serde::Deserialize<'de>,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
        S: Fn(K, V) -> Option<(K, V)>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name));
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(None);
            }
        }
        let table = table?;
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

    pub fn list_keys<'a, K>(&self, table_name: &str) -> Result<Vec<K>>
    where
        K: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name));
        // don't error if the table doesn't exist, simply return an empty vec
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(vec![]);
            }
        }
        let table = table?;
        let mut range = table.range::<Bytes>(..)?;
        let mut out = Vec::default();
        while let Some(item) = range.next() {
            let key = item?.0.value().parse()?;
            out.push(key);
        }
        Ok(out)
    }

    /// Scan a table for many matching keys. May not be invoked on a multimap table.
    pub fn find_many<'a, K, V, S>(&self, table_name: &str, selector: S) -> Result<Vec<(K, V)>>
    where
        K: serde::Serialize + for<'de> serde::Deserialize<'de>,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
        S: Fn(&K, &V) -> bool,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name));
        // don't error if the table doesn't exist, simply return an empty vec
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(vec![]);
            }
        }
        let table = table?;
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

    /// Get a value from a table. May not be invoked on a multimap table.
    pub fn get<K, V>(&self, table_name: &str, key: &K) -> Result<Option<V>>
    where
        K: serde::Serialize,
        V: for<'de> serde::Deserialize<'de>,
    {
        let read = self.db.begin_read()?;
        let table = read.open_table(TableDefinition::<Bytes, Bytes>::new(table_name));
        if let Err(e) = &table {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                return Ok(None);
            }
        }
        let table = table?;
        if let Some(val) = table.get(Bytes::encode(key)?)? {
            Ok(val.value().parse()?)
        } else {
            Ok(None)
        }
    }

    /// Count the number of keys present in a table.
    pub fn count<K, V>(&self, table_name: &str) -> Result<u64>
    where
        K: redb::Key + 'static,
        V: redb::Value + 'static,
    {
        let read = self.db.begin_read()?;
        match read.open_table(TableDefinition::<K, V>::new(table_name)) {
            Ok(table) => Ok(table.len()?),
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Remove a value from a table. May not be invoked on a multimap table.
    pub fn remove<K, V>(&self, table_name: &str, key: &K) -> Result<Option<V>>
    where
        K: serde::Serialize,
        V: for<'de> serde::Deserialize<'de>,
    {
        let mut tx = self.begin_write()?;
        let mut table = tx.open_table(table_name)?;
        let removed = table.remove(key)?.and_then(|v| Some(v.value()));
        drop(table);
        tx.commit()?;
        if let Some(removed) = removed {
            Ok(Some(removed.parse::<V>()?))
        } else {
            Ok(None)
        }
    }
}
