use redb::*;
use serde::Serialize;

use super::TransactionOperations;
use super::table::JournaledTable;

pub struct JournaledTransaction<K>
where
    K: Key + Clone + Serialize + 'static,
    for<'b> K::SelfType<'b>: ToOwned<Owned = K>,
{
    tx: WriteTransaction,
    journal_channel: (
        flume::Sender<TransactionOperations<K>>,
        flume::Receiver<TransactionOperations<K>>,
    ),
}

impl<'a, K> JournaledTransaction<K>
where
    K: Key + Clone + Serialize + 'static,
    for<'b> K::SelfType<'b>: ToOwned<Owned = K>,
{
    pub fn new(tx: WriteTransaction) -> Self {
        Self {
            tx,
            journal_channel: flume::unbounded(),
        }
    }

    pub fn open_table(
        &self,
        definition: TableDefinition<K, K>,
    ) -> Result<JournaledTable<K>, TableError> {
        let table = self.tx.open_table(definition)?;
        self.journal_channel
            .0
            .send(TransactionOperations::OpenTable(
                definition.name().to_string(),
            ))
            .unwrap();
        Ok(JournaledTable::new(table, self.journal_channel.0.clone()))
    }

    pub fn open_multimap_table<'txn>(
        &'txn self,
        definition: MultimapTableDefinition<K, K>,
    ) -> Result<MultimapTable<'txn, K, K>, TableError> {
        self.tx.open_multimap_table(definition)
    }

    pub fn rename_table(
        &self,
        definition: impl TableHandle,
        new_name: impl TableHandle,
    ) -> Result<(), TableError> {
        self.tx.rename_table(definition, new_name)
    }

    pub fn rename_multimap_table(
        &self,
        definition: impl MultimapTableHandle,
        new_name: impl MultimapTableHandle,
    ) -> Result<(), TableError> {
        self.tx.rename_multimap_table(definition, new_name)
    }

    pub fn delete_table(&self, definition: impl TableHandle) -> Result<bool, TableError> {
        self.tx.delete_table(definition)
    }

    pub fn delete_multimap_table(
        &self,
        definition: impl MultimapTableHandle,
    ) -> Result<bool, TableError> {
        self.tx.delete_multimap_table(definition)
    }

    pub fn list_tables(&self) -> Result<impl Iterator<Item = UntypedTableHandle> + '_> {
        self.tx.list_tables()
    }

    pub fn list_multimap_tables(
        &self,
    ) -> Result<impl Iterator<Item = UntypedMultimapTableHandle> + '_> {
        self.tx.list_multimap_tables()
    }

    pub fn commit(self) -> Result<Vec<TransactionOperations<K>>, CommitError> {
        self.tx.commit()?;
        Ok(self.journal_channel.1.drain().collect())
    }

    pub fn abort(self) -> Result {
        self.tx.abort()
    }
}
