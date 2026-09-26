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

/// A trait representing a document which may be queried.
pub trait Queryable: Clone + Send + Sync {
    type DocumentQuery: Default + Clone + Query + Send + Sync;
    /// Used to access the types of fields by name. Accessors generated in
    /// anondb-macros/src/document.rs
    type DocumentPhantom;

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
    fn owned_field_types() -> Vec<(String, String)> {
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

pub trait Maybe {
    type MaybeStruct;
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
        unimplemented!()
    }
}
