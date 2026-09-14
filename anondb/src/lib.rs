mod collection;
mod index;
mod index_compat;
mod lexicographic;
pub(crate) mod log;
mod metadata;
mod query;
mod query_params;
mod types;

use std::fmt::Display;

pub use collection::*;
pub use index::*;
pub use index_compat::*;
pub use lexicographic::*;
use metadata::*;
pub use query::*;
pub use query_params::*;
pub use types::*;

#[cfg(test)]
mod test;
#[cfg(test)]
use test::*;

use anondb_kv;

pub(crate) use anondb_kv::*;
pub use anondb_macros::AnonDB;
pub use anondb_macros::Document;

pub use rmp_serde;
pub use rmpv;

pub(crate) const NAME_SEPARATOR: &'static str = "___";

/// A trait representing a document which may be queried.
pub trait Queryable: Clone {
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

/// Trait for reflecting document types.
pub trait AnyDocument {
    /// Get an ordered list of document field names, with corresponding types.
    fn field_types() -> &'static Vec<(String, String)>;
    fn from_tuple(tuple: Vec<(String, rmpv::Value)>) -> Self;
}

pub trait Migrateable {
    fn migrations() -> &'static [fn(
        Vec<(String, rmpv::Value)>,
    ) -> Result<Vec<(String, rmpv::Value)>, MigrationError>] {
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

impl From<rmpv::decode::Error> for MigrationError {
    fn from(value: rmpv::decode::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<rmp_serde::encode::Error> for MigrationError {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl std::error::Error for MigrationError {
    fn description(&self) -> &str {
        unimplemented!()
    }
}
