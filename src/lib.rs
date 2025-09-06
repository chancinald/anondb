/// This library exposes a redb equivalent api
/// with a `Iter<JournaledTransaction>` that can be
/// replayed, merged, copied, etc.
///
use serde::Deserialize;
use serde::Serialize;

mod journal;
mod table;
mod transaction;

pub use journal::Journal;
pub use table::JournaledTable;
pub use transaction::JournaledTransaction;

pub type Bytes = &'static [u8];

const JOURNAL_TABLE: &str = "redb_journal";
const JOURNAL_STATE_KEY: Bytes = "redb_journal_state".as_bytes();

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
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
        key_bytes: Vec<u8>,
        value_bytes: Vec<u8>,
    },
    /// Remove all values associated with a key
    Remove(String, Vec<u8>),
    /// Write the transaction. May optionally be final element in a transaction.
    /// Transactions without a `Commit` should not be persisted to the db.
    Commit,
}
