use serde::Deserialize;
use serde::Serialize;

mod table;
mod transaction;

pub use table::JournaledTable;
pub use transaction::JournaledTransaction;

#[cfg(test)]
mod test;

/// We need to construct a single Key and Value implementation that wraps all relevant key and
/// value types so we can fulfill the `'static` lifetime parameter. To achieve this developers
/// should implement `Key` and `Value` types with each function containing a match clause.
#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionOperations<K: redb::Key + 'static> {
    OpenTable(String),
    OpenMultimapTable(String),
    Insert(String, K, K),
}

/// Each variant needs to be explicitly keyed to an underlying value. This is necessary to preserve
/// backward compatiblity if values are added or removed. This can be done automatically, but
/// rearranging the enum will silently break compat so it's written explicitly.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[repr(u8)]
pub enum DynValue {
    String(String) = 0,
    U32(u32) = 1,
}

impl redb::Key for DynValue {
    // TODO: actual comparison
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl redb::Value for DynValue {
    type SelfType<'a> = DynValue;
    type AsBytes<'a> = Vec<u8>;

    /// All variants must be dynamically sized to allow one variant to be dynamically sized.
    fn fixed_width() -> Option<usize> {
        None
    }

    /// Deserialize directly from a serde representation of the enum. The appropriate variant will
    /// automatically be returned.
    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bincode::deserialize(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        bincode::serialize(value).unwrap()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("DynValue")
    }
}
