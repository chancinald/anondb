use anyhow::Result;
use redb::*;

use crate::*;

pub struct ActiveTransaction<'tx> {
    tx: Option<WriteTransaction>,
    journal: &'tx Journal,
    operation_channel: (
        flume::Sender<TransactionOperation>,
        flume::Receiver<TransactionOperation>,
    ),
}

impl<'tx> ActiveTransaction<'tx> {
    pub fn new(journal: &'tx Journal) -> Result<Self> {
        Ok(Self {
            tx: Some(journal.db.begin_write()?),
            journal,
            operation_channel: flume::unbounded(),
        })
    }

    pub fn operate(&self, operation: TransactionOperation) -> Result<()> {
        self.operation_channel.0.send(operation)?;
        Ok(())
    }

    pub fn active_tx(&self) -> Result<&WriteTransaction> {
        self.tx
            .as_ref()
            .ok_or(anyhow::anyhow!("no active write transaction"))
    }

    pub fn open_table(&self, name: &str) -> Result<JournaledTable> {
        let table = self
            .active_tx()?
            .open_table::<Bytes, Bytes>(TableDefinition::new(name))?;
        self.operation_channel
            .0
            .send(TransactionOperation::OpenTable(name.into()))?;

        Ok(JournaledTable::new(table, self.journal, self))
    }

    pub fn commit(&mut self) -> Result<()> {
        let tx: WriteTransaction =
            std::mem::take(&mut self.tx).ok_or(anyhow::anyhow!("no active write transaction"))?;
        let mut operations = self.operation_channel.1.drain().collect::<Vec<_>>();
        operations.push(TransactionOperation::Commit);

        // journal state is always mutated by a transaction
        // it's not included in the Vec<TransactionOperation>
        //
        let state = self.journal.get_state()?;

        let mut journal_table = tx.open_table(JOURNAL_TABLE)?;
        let mut tx_table = tx.open_table(TX_TABLE)?;

        let journal_tx = JournalTransaction {
            last_tx_hash: state.last_tx_hash,
            operations,
        };
        let tx_hash = journal_tx.hash()?;

        tx_table.insert(tx_hash, Bytes::encode(&journal_tx)?)?;
        journal_table.insert(state.next_tx_index, tx_hash)?;

        drop(journal_table);
        drop(tx_table);

        tx.commit()?;

        self.journal.register_transaction(journal_tx)?;

        Ok(())
    }

    pub fn abort(&mut self) -> Result<()> {
        std::mem::take(&mut self.tx);
        self.operation_channel.1.drain();
        Ok(())
    }

    pub fn open_multimap_table(&'tx self, name: &str) -> Result<MultimapTable<'tx, Bytes, Bytes>> {
        let table = self
            .active_tx()?
            .open_multimap_table::<Bytes, Bytes>(MultimapTableDefinition::new(name))?;
        unimplemented!();
    }

    pub fn rename_table(&self, old_table_name: &str, new_table_name: &str) -> Result<()> {
        self.active_tx()?.rename_table(
            TableDefinition::<Bytes, Bytes>::new(old_table_name),
            TableDefinition::<Bytes, Bytes>::new(new_table_name),
        )?;
        self.operate(TransactionOperation::RenameTable(
            old_table_name.into(),
            new_table_name.into(),
        ))?;
        Ok(())
    }

    pub fn rename_multimap_table(&self, old_table_name: &str, new_table_name: &str) -> Result<()> {
        self.active_tx()?.rename_multimap_table(
            MultimapTableDefinition::<Bytes, Bytes>::new(old_table_name),
            MultimapTableDefinition::<Bytes, Bytes>::new(new_table_name),
        )?;
        self.operate(TransactionOperation::RenameMultimapTable(
            old_table_name.into(),
            new_table_name.into(),
        ))?;
        Ok(())
    }

    pub fn delete_table(&self, name: &str) -> Result<bool> {
        let out = self
            .active_tx()?
            .delete_table(TableDefinition::<Bytes, Bytes>::new(name))?;
        self.operate(TransactionOperation::DeleteTable(name.into()))?;
        Ok(out)
    }

    pub fn delete_multimap_table(&self, name: &str) -> Result<bool> {
        unimplemented!();
        let out = self
            .active_tx()?
            .delete_multimap_table(MultimapTableDefinition::<Bytes, Bytes>::new(name))?;
        self.operate(TransactionOperation::DeleteMultimapTable(name.into()))?;
        Ok(out)
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let read = self.journal.db.begin_read()?;
        Ok(read
            .list_tables()?
            .map(|table| table.name().to_string())
            .collect::<Vec<_>>())
    }
}
