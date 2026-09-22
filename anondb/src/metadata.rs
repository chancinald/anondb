use serde::Deserialize;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexDescription {
    /// The names of the fields in the index
    pub field_names: Vec<String>,
    pub options: IndexOptions,
    pub table_name: String,
}

/// A type for dynamic reflection of collections.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CollectionDescription {
    /// Currently stored version of the documents in the collection.
    pub version: u32,
    /// field name along with type description string
    pub fields: Vec<(String, String)>,
    pub primary_key_fields: Vec<(String, LexStats)>,
    /// last known indices
    pub indices: Vec<IndexDescription>,
}
