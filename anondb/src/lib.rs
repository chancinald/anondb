mod collection;
mod index;
mod index_compat;
mod lexicographic;
mod metadata;
mod query;

pub use collection::*;
pub use index::*;
pub use index_compat::*;
pub use lexicographic::*;
use metadata::*;
pub use query::*;

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
}

#[cfg(test)]
pub fn rand_utf8(len: usize) -> String {
    vec![char::default(); len]
        .into_iter()
        .map(|_| rand::random::<char>())
        .collect()
}
