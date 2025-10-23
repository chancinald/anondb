mod collection;
mod index;
mod metadata;
mod query;

pub use collection::*;
pub use index::*;
use metadata::*;
pub use query::*;

#[cfg(test)]
mod test;

// re-exports
pub use anondb_kv::*;
pub use anondb_macros::AnonDB;
pub use anondb_macros::Document;

pub trait Queryable {
    type DocumentQuery: Default;

    fn query() -> Self::DocumentQuery {
        Self::DocumentQuery::default()
    }

    /// Test if a document matches a query
    fn matches(&self, query: &Self::DocumentQuery) -> bool;
}
