use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::*;
use anondb_kv::*;

pub trait Queryable {
    type Document;
}

impl<T, K: KV> Queryable for Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    type Document = T;
}

pub struct Collection<T, K: KV>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    kv: Option<Arc<K>>,
    name: Option<String>,
    /// Indices without names. Necessary to account for proc-macro function invocation after struct
    /// construction.
    indices: Vec<Arc<Index<T>>>,
    /// Extractor function to get a primary key from an instance of T
    primary_key_index: Option<Arc<Index<T>>>,
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
            primary_key_index: None,
            indices: Vec::default(),
        }
    }

    /// Returns `true` if a primary key has been set for this collection.
    pub fn has_primary_key(&self) -> bool {
        self.primary_key_index.is_some()
    }

    /// A function to set the primary key without consuming `self`. Used in the AnonDB proc macro.
    pub fn set_primary_key(&mut self, primary_key: (Vec<String>, fn(&T) -> Vec<u8>)) -> Result<()> {
        if self.primary_key_index.is_some() {
            anyhow::bail!(
                "Collection \"{}\" attempting to assign primary key twice!",
                self.name()
            );
        }
        self.primary_key_index = Some(Arc::new(Index {
            collection_name: self.name().to_string(),
            field_names: primary_key.0,
            serialize: primary_key.1,
            options: IndexOptions {
                unique: true,
                full_docs: true,
            },
        }));
        Ok(())
    }

    pub fn primary_key_index(&self) -> &Arc<Index<T>> {
        self.primary_key_index
            .as_ref()
            .expect("No primary key index set!")
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
    pub(crate) fn kv(&self) -> &Arc<K> {
        self.kv
            .as_ref()
            .expect(&format!("Collection \"{}\" has no kv set!", self.name()))
    }

    /// Get a reference to indices associated with this collection, keyed to their kv table name.
    pub fn indices(&self) -> &Vec<Arc<Index<T>>> {
        &self.indices
    }

    /// Get a reference to the primary key extractor.
    fn primary_key_extractor(&self) -> &fn(&T) -> Vec<u8> {
        self.primary_key_index
            .as_ref()
            .map(|index| &index.serialize)
            .expect(&format!(
                "Collection \"{}\" has no primary key set!",
                self.name()
            ))
    }

    /// Define an index with a `name`, and a set of fields and their sort direction. This will
    /// create an index over 1 or more fields. See kv.rs for information on how indices are sorted.
    /// Indices are automatically used during operation and can be lazily initialized/removed.
    pub fn add_index(&mut self, index: Index<T>) -> Result<()> {
        if index.field_names.is_empty() {
            log::warn!(
                "In collection \"{}\", index \"{}\" contains no fields",
                self.name(),
                index.table_name()
            );
            #[cfg(not(debug_assertions))]
            panic!("Refusing to start in production mode with an empty index");
        }
        self.indices.push(Arc::new(index));
        Ok(())
    }

    /// Take the vector of indices and check for consistency.
    /// This should be automatically invoked by the AnonDB proc macro.
    pub fn construct_indices(&mut self) -> Result<()> {
        let mut known_indices = HashMap::<String, ()>::default();
        for index in &self.indices {
            // check that the index collection name matches our name
            if index.collection_name != self.name() {
                anyhow::bail!(
                    "In collection \"{}\", index \"{}\" has a mismatched collection name",
                    self.name(),
                    index.table_name()
                );
            }
            // check that the index has at least 1 field
            if index.field_names.is_empty() {
                log::warn!(
                    "In collection \"{}\", index \"{}\" contains no fields",
                    self.name(),
                    index.table_name()
                );
                #[cfg(not(debug_assertions))]
                panic!("Refusing to start in production mode with an empty index");
            }
            // make sure there are no duplicate indices
            let name = index.table_name();
            if known_indices.contains_key(&name) {
                anyhow::bail!(
                    "Collection \"{}\" contains a duplicate index: \"{name}\"",
                    self.name()
                );
            }
            known_indices.insert(name, ());
        }
        Ok(())
    }

    /// Return all the table names that this collection uses in the underlying KV.
    pub fn table_names(&self) -> Vec<String> {
        vec![
            vec![self.name().to_string()],
            self.indices()
                .iter()
                .map(|index| index.table_name())
                .collect::<Vec<_>>(),
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
        for index in &self.indices {
            index.insert(&tx, document, &primary_key)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Clear all indices for this collection and completely rebuild them. This operation is `O(N)`
    /// over the number of documents in the collection.
    pub fn rebuild_indices(&self) -> Result<()> {
        // first empty all index collections
        let tx = self.kv().write_tx()?;
        for index in &self.indices {
            tx.clear(&index.table_name())?;
        }
        tx.commit()?;

        // then iterate over all documents and construct indices
        self.kv().scan(self.name(), |primary_key, val| {
            let data = rmp_serde::from_slice(val)?;
            for index in &self.indices {
                let key = (index.serialize)(&data);
                // we insert the document primary key as the value and the indexed bytes as the
                // key to get lexicographic iteration
                self.kv()
                    .insert(&index.table_name(), key.as_slice(), primary_key)?;
            }
            return Ok(true);
        })?;
        Ok(())
    }

    pub fn find_many(&self, query: Query<T>) -> Result<impl Iterator<Item = T>> {
        let primary_index_score = self.primary_key_index().query_compat(&query)?;

        let mut scores = BTreeMap::default();
        for index in self.indices() {
            let score = index.query_compat(&query)?;
            scores.insert(score, index.clone());
        }
        let (score, best_index) = scores
            .last_key_value()
            .map(|(k, v)| (*k, v.clone()))
            .ok_or(anyhow::anyhow!("no index found"))?;
        println!(
            "using index \"{}\" score: {}",
            best_index.table_name(),
            score
        );
        let tx = self.kv().read_tx()?;
        Ok(best_index
            .query(&tx, query)?
            .collect::<Vec<_>>()
            .into_iter())
    }

    pub fn find_one(&self, query: Query<T>) -> Result<Option<T>> {
        let mut scores = BTreeMap::default();
        for index in self.indices() {
            let score = index.query_compat(&query)?;
            scores.insert(score, index.clone());
        }
        if let Some((score, best_index)) = scores.last_key_value() {
            println!(
                "using index \"{}\" score: {}",
                best_index.table_name(),
                score
            );
            let tx = self.kv().read_tx()?;
            let mut out = best_index.query(&tx, query)?;
            Ok(out.next())
        } else {
            unreachable!("no index found!");
        }
    }
}

impl<T, K: KV> Default for Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    fn default() -> Self {
        Self {
            // these none values will be assigned in the anondb_macros::AnonDB derive macro
            // they should be Some immediately after startup
            kv: None,
            name: None,
            primary_key_index: None,
            indices: Vec::default(),
        }
    }
}
