mod collection;
mod index;
mod index_compat;
mod lexicographic;
pub(crate) mod log;
mod metadata;
mod primary_index;
mod query;
mod query_params;
mod types;

use std::collections::HashMap;
use std::fmt::Display;

pub use collection::*;
pub use index::*;
pub use index_compat::*;
pub use lexicographic::*;
pub use metadata::*;
pub use primary_index::*;
pub use query::*;
pub use query_params::*;
pub use types::*;

#[cfg(test)]
mod test;
#[cfg(test)]
use test::*;

use anondb_kv;

pub use anondb_kv::*;
pub use anondb_macros::AnonDB;
pub use anondb_macros::Document;
pub mod result {
    pub use anyhow::Result;
}

pub use futures;
pub use postcard;

pub(crate) const NAME_SEPARATOR: &'static str = "___";

pub trait CollectionDocument:
    AutoDocumentTypes + Queryable + AnyDocument + ParseableDocument + Migrateable
{
}

pub trait AutoDocumentTypes
where
    Self: Sized,
{
    /// Used to access the types of fields by name. Accessors generated in
    /// anondb-macros/src/document.rs
    type DocumentPhantom;
    /// For static incremental encoding and decoding.
    /// This type has the exact same fields as the document, but with
    /// each field wrapped in Option<>
    type MaybeStruct: MaybeSer + Into<Self> + From<Self>;
}

pub trait MaybeSer {
    fn serialize_remaining(self) -> postcard::Result<Vec<u8>>;
    fn deserialize_remaining(&mut self, bytes: &[u8]) -> postcard::Result<()>;
    fn serialize_remaining_to_vec(self) -> Vec<(String, String, Box<dyn ValidDBType>)>;
}

pub trait RunnableQuery<'a, P> {
    fn new(provider: &'a P) -> Self;
}

pub trait QueryFor<T: CollectionDocument + 'static, P: WriteQueryOperations<T> + Send + Sync> {
    fn query_for<'a>(&'a self) -> T::RunnableDocumentQuery<'a, P>;
}

/// A trait representing a document which may be queried.
pub trait Queryable: Clone + Send + Sync {
    type DocumentQuery: Clone + Default + Send + Sync + Into<Query>;
    type RunnableDocumentQuery<'a, P: WriteQueryOperations<Self> + Send + Sync + 'static>: RunnableQuery<'a, P>
        + Send
        + Sync
    where
        Self: CollectionDocument;

    fn query() -> Self::DocumentQuery {
        Self::DocumentQuery::default()
    }

    /// Test if a document matches a query
    fn matches(&self, query: &Self::DocumentQuery) -> bool;
}

/// Trait for reflecting document types.
pub trait AnyDocument {
    /// Get an ordered list of document field names, with corresponding types.
    fn field_types() -> &'static [(&'static str, &'static str)];
    fn owned_field_types() -> HashMap<String, String> {
        Self::field_types()
            .iter()
            .map(|v| (v.0.into(), v.1.into()))
            .collect()
    }
}

pub trait ParseableDocument {
    /// Convert to a hashmap of field names keyed to field type and type erased value.
    fn into_map(self) -> std::collections::BTreeMap<String, (String, Box<dyn ValidDBType>)>;
}

pub trait Migrateable {
    fn migrations<'a>() -> &'static [fn(
        Vec<(String, Vec<u8>)>,
        Vec<String>,
        Vec<u8>,
    ) -> Result<
        (Vec<(String, Vec<u8>)>, Vec<String>, Vec<u8>),
        MigrationError,
    >] {
        &[]
    }
}

/// A trait for handling changes to a database schema.
#[derive(Debug)]
pub struct MigrationError {
    message: String,
}

impl Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for MigrationError {
    fn description(&self) -> &str {
        self.message.as_str()
    }
}
