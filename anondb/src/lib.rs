mod collection;
mod index;
mod index_compat;
mod lexicographic;
mod metadata;
mod query;
mod types;

pub use collection::*;
pub use index::*;
pub use index_compat::*;
pub use lexicographic::*;
use metadata::*;
pub use query::*;
pub use types::*;

#[cfg(test)]
mod test;

use anondb_kv;

pub use anondb_kv::*;
pub use anondb_macros::AnonDB;
pub use anondb_macros::Document;

pub trait Query {
    /// Get the fields that are set in this query.
    fn fields(&self) -> Vec<(&'static str, Param)>;
}

/// A trait representing a document which may be queried.
pub trait Queryable {
    type DocumentQuery: Default + Clone + Query + 'static;
    /// Used to access the types of fields by name. Accessors generated in
    /// anondb-macros/src/document.rs
    type DocumentPhantom;

    fn query() -> Self::DocumentQuery {
        Self::DocumentQuery::default()
    }

    /// Test if a document matches a query
    fn matches(&self, query: &Self::DocumentQuery) -> bool;

    fn field_names() -> Vec<String> {
        unimplemented!()
    }
}

use std::collections::BTreeMap;
pub(crate) const METADATA_TABLE_NAME: &str = "______anondb_metadata";
pub trait DB<K: KV>: Sized {
    /// Initialize the database by checking collection and index consistency.
    fn init_db(
        self,
        kv: K,
        collections: BTreeMap<String, IndexDescription>,
    ) -> impl Future<Output = anyhow::Result<Self>> {
        async move {
            let metadata = kv.get(METADATA_TABLE_NAME, &[0]).await?;
            Ok(self)
        }
    }
}

#[cfg(test)]
pub fn rand_utf8(len: usize) -> String {
    vec![char::default(); len]
        .into_iter()
        .map(|_| rand::random::<char>())
        .collect()
}
