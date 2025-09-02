use serde::Deserialize;
use serde::Serialize;

mod table;
mod transaction;

pub use table::JournaledTable;
pub use transaction::JournaledTransaction;

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionOperations {
    OpenTable(String),
    OpenMultimapTable(String),
    Insert(String, Vec<u8>, Vec<u8>),
    Remove(String, Vec<u8>),
    Commit(),
}
