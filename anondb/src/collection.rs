use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::*;
use anondb_kv::*;

pub struct Collection<T, K: KV>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    kv: Option<Arc<K>>,
    name: Option<String>,
    /// KV table name keyed to the index data
    named_indices: Option<HashMap<String, Index<T>>>,
    /// Indices without names. Necessary to account for proc-macro function invocation after struct
    /// construction.
    indices: Vec<Index<T>>,
    /// Extractor function to get a primary key from an instance of T
    primary_key: Option<(String, fn(&T) -> Vec<u8>)>,
}

impl<T, K: KV> Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Initialize a new collection
    pub fn new() -> Self {
        Self {
            // these none values will be assigned in the anondb_macros::AnonDB derive macro
            kv: None,
            name: None,
            primary_key: None,
            named_indices: None,
            indices: Vec::default(),
        }
    }

    /// Returns `true` if a primary key has been set for this collection.
    pub fn has_primary_key(&self) -> bool {
        self.primary_key.is_some()
    }

    /// Set a primary key extractor for the collection.
    /// Failing to set a primary key extractor will cause a runtime error.
    pub fn set_primary_key(mut self, primary_key: (String, fn(&T) -> Vec<u8>)) -> Result<Self> {
        if self.primary_key.is_some() {
            anyhow::bail!(
                "Collection \"{}\" attempting to assign primary key twice!",
                self.name()
            );
        }
        self.primary_key = Some(primary_key);
        Ok(self)
    }

    /// A function to set the primary key without consuming `self`. Used in the AnonDB proc macro.
    pub fn set_primary_key_nonbuilder(
        &mut self,
        primary_key: (String, fn(&T) -> Vec<u8>),
    ) -> Result<()> {
        if self.primary_key.is_some() {
            anyhow::bail!(
                "Collection \"{}\" attempting to assign primary key twice!",
                self.name()
            );
        }
        self.primary_key = Some(primary_key);
        Ok(())
    }

    /// Set the name of the collection. This should be automatically invoked by the AnonDB proc
    /// macro.
    pub fn set_name(&mut self, name: String) -> Result<()> {
        if self.name.is_some() {
            anyhow::bail!(
                "Collection \"{}\" attempting to assign name twice! Second name: \"{name}\"",
                self.name()
            );
        }
        self.name = Some(name);
        Ok(())
    }

    /// Set the backing KV used for this collection. This should be automatically invoked by the
    /// AnonDB proc macro.
    pub fn set_kv(&mut self, kv: Arc<K>) -> Result<()> {
        if self.kv.is_some() {
            anyhow::bail!(
                "Collection \"{}\" attempting to assign kv twice!",
                self.name()
            );
        }
        self.kv = Some(kv);
        Ok(())
    }

    /// Get a reference to the backing KV.
    fn kv(&self) -> &Arc<K> {
        self.kv
            .as_ref()
            .expect(&format!("Collection \"{}\" has no kv set!", self.name()))
    }

    /// Get a reference to indices associated with this collection, keyed to their kv table name.
    fn indices(&self) -> &HashMap<String, Index<T>> {
        self.named_indices
            .as_ref()
            .expect("Collection has not constructed \"named_indices\".")
    }

    /// Get a reference to the primary key extractor.
    fn primary_key_extractor(&self) -> &fn(&T) -> Vec<u8> {
        self.primary_key
            .as_ref()
            .map(|(_, extractor)| extractor)
            .expect(&format!(
                "Collection \"{}\" has no primary key set!",
                self.name()
            ))
    }

    #[allow(dead_code)]
    fn primary_key(&self) -> &(String, fn(&T) -> Vec<u8>) {
        self.primary_key.as_ref().expect(&format!(
            "Collection \"{}\" has no primary key set!",
            self.name()
        ))
    }

    /// Define an index with a `name`, and a set of fields and their sort direction. This will
    /// create an index over 1 or more fields. See kv.rs for information on how indices are sorted.
    /// Indices are automatically used during operation and can be lazily initialized/removed.
    pub fn add_index(mut self, index: Index<T>) -> Result<Self> {
        if index.field_names.is_empty() {
            log::warn!(
                "In collection \"{}\", index \"{}\" contains no fields",
                self.name(),
                index.id()
            );
            #[cfg(not(debug_assertions))]
            panic!("Refusing to start in production mode with an empty index");
        }
        self.indices.push(index);
        Ok(self)
    }

    /// Take the vector of indices and build a hashmap with proper table names.
    /// This should be automatically invoked by the AnonDB proc macro.
    pub fn construct_indices(&mut self) -> Result<()> {
        let mut named_indices = HashMap::default();
        for index in std::mem::take(&mut self.indices) {
            let name = format!("{}_{}", self.name(), index.id());
            if index.field_names.len() == 0 {
                log::warn!(
                    "In collection \"{}\", index \"{}\" contains no fields",
                    self.name(),
                    index.id()
                );
                #[cfg(not(debug_assertions))]
                panic!("Refusing to start in production mode with an empty index");
            }
            if named_indices.contains_key(&name) {
                anyhow::bail!(
                    "Collection \"{}\" contains a duplicate index: \"{name}\"",
                    self.name()
                );
            }
            named_indices.insert(name, index);
        }
        self.named_indices = Some(named_indices);
        Ok(())
    }

    /// Return all the table names that this collection uses in the underlying KV.
    pub fn table_names(&self) -> Vec<String> {
        vec![
            vec![self.name().to_string()],
            self.indices().keys().cloned().collect::<Vec<_>>(),
        ]
        .concat()
    }

    /// The name of the collection. This is the name of the table that will be used in the
    /// underlying KV.
    pub fn name(&self) -> &str {
        self.name
            .as_ref()
            .expect("Collection does not have a name set!")
    }

    /// Return the number of documents in the collection.
    pub fn count(&self) -> Result<u64> {
        self.kv().count(self.name())
    }

    /// Insert a document into a collection. All relevant indices will be updated.
    pub fn insert(&self, document: &T) -> Result<()> {
        // Serialize our document
        let data = rmp_serde::to_vec_named(document)?;

        let tx = self.kv().write_tx()?;
        // Serialize our document primary key
        let primary_key = (self.primary_key_extractor())(document);
        // Check if the primary key exists, if so reject the insertion
        // TODO: use a "contains_key" type function to avoid loading the data unnecessarily
        if tx.get(self.name(), primary_key.as_slice())?.is_some() {
            anyhow::bail!(
                "Attempting to insert document with duplicate primary key in collection \"{}\": primary key: \"{:?}\"",
                self.name(),
                primary_key
            );
        }
        tx.insert(self.name(), primary_key.as_slice(), data.as_slice())?;
        for (table_name, index) in self.indices() {
            // For each index serialize a key for the index
            let key = (index.serialize)(document);
            if index.options.unique {
                // If the index is unique we need to reject duplicate keys
                if tx.get(&table_name, key.as_slice())?.is_some() {
                    anyhow::bail!(
                        "Collection \"{}\" index \"{}\" cannot insert document, uniqueness constraint violated",
                        self.name(),
                        index.name
                    );
                }
                tx.insert(&table_name, key.as_slice(), primary_key.as_slice())?;
            } else {
                // non-unique index, we're inserting into a multimap table
                tx.insert_multimap(&table_name, key.as_slice(), primary_key.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Clear all indices for this collection and completely rebuild them. This operation is `O(N)`
    /// over the number of documents in the collection.
    pub fn rebuild_indices(&self) -> Result<()> {
        // first empty all index collections
        let tx = self.kv().write_tx()?;
        for (name, _index) in self.indices() {
            tx.clear(name)?;
        }
        tx.commit()?;

        // then iterate over all documents and construct indices
        self.kv().scan(self.name(), |primary_key, val| {
            let data = rmp_serde::from_slice(val)?;
            for (name, index) in self.indices() {
                let key = (index.serialize)(&data);
                // we insert the document primary key as the value and the indexed bytes as the
                // key to get lexicographic iteration
                self.kv().insert(name, key.as_slice(), primary_key)?;
            }
            return Ok(true);
        })?;
        Ok(())
    }
}

impl<T, K: KV> Default for Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    fn default() -> Self {
        Self {
            // these none values will be assigned in the anondb_macros::AnonDB derive macro
            kv: None,
            name: None,
            primary_key: None,
            named_indices: None,
            indices: Vec::default(),
        }
    }
}
