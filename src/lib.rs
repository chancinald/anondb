/// This library exposes a redb equivalent api
/// with a `Iter<JournaledTransaction>` that can be
/// replayed, merged, copied, etc.
///
use serde::Deserialize;
use serde::Serialize;

mod bytes;
mod journal;
mod table;
mod transaction;

pub use bytes::Bytes;
pub use journal::Journal;
pub use table::JournaledTable;
pub use transaction::JournaledTransaction;

const JOURNAL_TABLE: &str = "redb_journal";
const JOURNAL_STATE_KEY: &'static [u8] = "redb_journal_state".as_bytes();

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalTransaction {
    pub operations: Vec<TransactionOperation>,
    pub index: u64,
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
