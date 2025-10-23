use std::collections::BTreeMap;
use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Serialize, Deserialize, PartialEq)]
pub struct IndexDescription {
    pub collection_name: String,
    pub field_names: Vec<String>,
    pub options: IndexOptions,
    /// Name of the table in the kv used for this collection
    pub table_name: String,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct CollectionDescription {
    pub fields: BTreeMap<String, String>,
}

/// Represents metadata about an AnonDB instance. Tracks changes to collection document shapes and indices.
///
/// Determines what work needs to be done on startup, and if stable startup is possible after
/// schema changes.
#[derive(Serialize, Deserialize)]
pub struct MetadataDocument {
    pub version: u64,        // version of the metadata document itself
    pub schema_version: u64, // version of the database schema, this gets incremented when the
    // schema changes
    pub indices_by_collection: HashMap<String, IndexDescription>,
    pub collections: HashMap<String, CollectionDescription>,
}

impl MetadataDocument {
    /// We'll compute a metadata document based on the current instantiation of AnonDB, and compare
    /// with any previously stored metadata to determine schema changes.
    pub fn compare(&self, other: &Self) -> Result<()> {
        if self.version != other.version {
            anyhow::bail!(
                "Metadata version mimatch. This implementation of AnonDB does not support metadata version migration."
            );
        }
        if self.indices_by_collection == other.indices_by_collection
            && self.collections == other.collections
        {
            return Ok(());
        }

        // otherwise we have changes to the schema. We need to determine changes to indices and
        // remove/rebuild them as necessary
        //
        // and examine changes to collection descriptions (documents). If the changes are purely
        // additive we assume the user specifies a default impl for backward compat.
        //
        // If there are changes to the type of a field we need to reject it without a custom migration

        anyhow::bail!("Changes to schema detected, refusing to start");
    }
}
