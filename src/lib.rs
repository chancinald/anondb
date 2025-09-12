/// This library exposes a redb equivalent api
/// with a `Iter<JournaledTransaction>` that can be
/// replayed, merged, copied, etc.
///
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

mod active_transaction;
mod bytes;
mod journal;
mod multimap_table;
mod table;

pub use active_transaction::ActiveTransaction;
pub use bytes::Bytes;
pub use journal::Journal;
pub use table::JournaledTable;

/// Stores the sequence of transactions that have been applied. Transaction index
/// keyed to hash of the transaction.
const JOURNAL_TABLE_NAME: &str = "_______anondb_journal";
const JOURNAL_TABLE: redb::TableDefinition<u64, [u8; 32]> =
    redb::TableDefinition::new(JOURNAL_TABLE_NAME);
/// Stores the transaction data. Each transaction is stored keyed to its hash `<[u8; 32],
/// JournalTransaction>`.
const TX_TABLE_NAME: &str = "_______anondb_transactions";
const TX_TABLE: redb::TableDefinition<[u8; 32], Bytes> = redb::TableDefinition::new(TX_TABLE_NAME);

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalTransaction {
    pub last_tx_hash: [u8; 32],
    pub operations: Vec<TransactionOperation>,
}

impl JournalTransaction {
    pub fn hash(&self) -> Result<[u8; 32]> {
        Ok(blake3::hash(Bytes::encode(&self)?.as_slice()).into())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum TransactionOperation {
    /// Rename a table
    RenameTable(String, String),
    RenameMultimapTable(String, String),
    DeleteTable(String),
    DeleteMultimapTable(String),
    /// Open a table for mutation by name
    OpenTable(String),
    /// Open a table allowing multiple entries per key
    OpenMultimapTable(String),
    /// Insert a value for key. Existing value is overwritten or (in multimap) expanded by concatenation
    Insert {
        table_name: String,
        key_bytes: Bytes,
        value_bytes: Bytes,
    },
    /// Remove all values associated with a key
    Remove(String, Bytes),
    /// Write the transaction. May optionally be final element in a transaction.
    /// Transactions without a `Commit` should not be persisted to the db.
    Commit,
}
