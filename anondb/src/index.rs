use std::collections::HashMap;
use std::ops::Bound;
use std::ops::RangeBounds;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use anondb_kv::*;

use crate::*;

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct IndexOptions {
    pub unique: bool,    // only allow 1 unique combination of each field in the index
    pub full_docs: bool, // does the index store the full document, or just the primary key?
}

// TODO: explicitly check and disallow duplicate field names
#[derive(Debug, Clone, PartialEq)]
pub struct Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    /// The name of the collection the index belongs to.
    pub collection_name: String,
    /// The field names of the document type.
    pub field_names: Vec<String>,
    /// Take a document of type `T` and serialize it into a lexicographically sortable key
    pub serialize: fn(&T) -> Vec<u8>,
    /// Options for the index
    pub options: IndexOptions,
}

impl<T> Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    /// Name of the table in the kv where the index will be stored. This should be a combination of
    /// the collection name and the fields being indexed.
    pub fn table_name(&self) -> String {
        format!(
            "{}_{}{}",
            self.collection_name,
            self.field_names.join("_"),
            if self.options.unique { "_unique" } else { "" }
        )
    }

    /// Accept a set of field keys as lexicographically serialized bytes
    pub fn query<'tx>(
        &self,
        tx: &'tx impl ReadOperations,
        query: &T::DocumentQuery,
        index_fields: &HashMap<String, Param>,
    ) -> Result<impl Iterator<Item = T>> {
        let mut min_key = LexicographicKey::default();
        let mut min_bound: Bound<Vec<u8>> = Bound::Unbounded;
        let mut max_bound: Bound<Vec<u8>> = Bound::Unbounded;
        for name in &self.field_names {
            if let Some(query_param) = index_fields.get(name) {
                // at this point min_key must be either
                // 1. An empty vector
                // 2. An exact match for some leading fields
                match query_param {
                    Param::Eq(v) => {
                        min_key.append_key_slice(v);
                    }
                    Param::In(_v) => {
                        unimplemented!()
                    }
                    Param::Nin(_) | Param::Neq(_) => {
                        // not equal and not in operators cannot be accelerated by indices and must
                        // always be scanned
                        if !min_key.is_empty() {
                            min_bound = Bound::Included(min_key.bytes.clone());
                            min_key.append_upper_inclusive_byte();
                            max_bound = Bound::Included(min_key.take());
                        }
                        break;
                    }
                    Param::Range(v) => {
                        match v.start_bound() {
                            Bound::Unbounded => {
                                if !min_key.is_empty() {
                                    min_bound = Bound::Included(min_key.take());
                                } else {
                                    // otherwise min_bound is unbounded
                                }
                            }
                            Bound::Included(v) => {
                                min_key.append_key_slice(&v);
                                min_bound = Bound::Included(min_key.take());
                            }
                            Bound::Excluded(v) => {
                                // this doesn't always exclude v because all longer
                                // keys will sort after v. So if we're using only part of this
                                // index this bound will not be exclusive
                                //
                                // extraneous documents will be filtered in second stage
                                min_key.append_key_slice(&v);
                                min_bound = Bound::Excluded(min_key.take());
                            }
                        }
                        match v.end_bound() {
                            Bound::Unbounded => {
                                let mut max_key = min_key.clone();
                                max_key.append_upper_inclusive_byte();
                                max_bound = Bound::Included(max_key.take());
                            }
                            Bound::Included(v) => {
                                let mut max_key = min_key.clone();
                                max_key.append_key_slice(&v);
                                max_key.append_upper_inclusive_byte();
                                max_bound = Bound::Included(max_key.take());
                            }
                            Bound::Excluded(v) => {
                                // again, we can't exclude v, it will naturally be included because
                                // the upper inclusive byte forces longer byte vectors to sort
                                // before the max_key to support partial index use
                                //
                                // extraneous documents will be filtered in second stage
                                let mut max_key = min_key.clone();
                                max_key.append_key_slice(&v);
                                // TODO: check if we're on the last field of the index and don't
                                // append this to avoid extraneous document retrieval on full index
                                // use
                                max_key.append_upper_inclusive_byte();
                                max_bound = Bound::Excluded(max_key.take());
                            }
                        }
                        break;
                    }
                }
            } else {
                // we're using only part of the index, we'll suffix the min and max bounds with a
                // byte that will include all subsequent entries
                if !min_key.is_empty() {
                    // we have an exact partial filter on our index, create a range from it
                    min_bound = Bound::Included(min_key.bytes.clone());
                    min_key.append_upper_inclusive_byte();
                    max_bound = Bound::Included(min_key.take());
                }
                break;
            }
        }
        let scan_range = GeneralRange(min_bound, max_bound);
        let table_name = self.table_name();
        let docs = if self.options.unique {
            tx.range_buffered(&table_name, scan_range.as_slice(), |_k, v, _done| {
                // v represents the primary key, we'll load the document and check it against the
                // selector
                let doc_bytes = if self.options.full_docs {
                    v.to_vec()
                } else {
                    tx.get(&self.collection_name, v)?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Index \"{table_name}\" referencing primary key that does not exist!"
                        )
                    })?
                };
                // parse the bytes
                let doc = rmp_serde::from_slice::<T>(&doc_bytes)?;
                if doc.matches(query) {
                    Ok(Some(doc))
                } else {
                    Ok(None)
                }
            })?
        } else {
            tx.range_buffered_multimap(&table_name, scan_range.as_slice(), |_k, v, _done| {
                // multimap index never stores full documents, always load from the primary table
                let doc_bytes = tx.get(&self.collection_name, v)?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Index \"{table_name}\" referencing primary key that does not exist!"
                    )
                })?;
                // parse the bytes
                let doc = rmp_serde::from_slice::<T>(&doc_bytes)?;
                if doc.matches(query) {
                    Ok(Some(doc))
                } else {
                    Ok(None)
                }
            })?
        };
        Ok(docs.into_iter())
    }

    /// Determine how compatible this index is with a given query. A higher score indicates a
    /// faster query. An index that matches exactly returns a high score. An index that provides
    /// no acceleration returns 0.
    pub fn query_compat(
        &self,
        _query: &T::DocumentQuery,
        index_params: &HashMap<String, Param>,
    ) -> Result<usize> {
        let mut is_full_prefix = true; // are we able to utilize all of the fields in this index?
        let mut score: usize = 0;
        for (i, name) in self.field_names.iter().enumerate() {
            // is this the final field in the index?
            let is_last_field = i == self.field_names.len() - 1;
            if let Some(query_param) = index_params.get(name) {
                score += 1;
                match query_param {
                    Param::Eq(_) => {
                        score = score.saturating_mul(10);
                    }
                    Param::In(_) => {
                        // an In operator requires a constant number of reads
                        score = score.saturating_mul(8);
                    }
                    Param::Range(_) => {
                        score = score.saturating_mul(5);
                        if !is_last_field {
                            // if we have a range on a field that is not the last field in the
                            // index we don't consider this a full prefix match, because we'll have
                            // to iterate over other fields as well
                            is_full_prefix = false;
                        }
                        break; // break when we encounter a field that would necessitate a scan
                    }
                    Param::Neq(_) => {
                        score = score.saturating_mul(2);
                        if !is_last_field {
                            is_full_prefix = false;
                        }
                        break;
                    }
                    Param::Nin(_) => {
                        score = score.saturating_mul(2);
                        if !is_last_field {
                            is_full_prefix = false;
                        }
                        break;
                    }
                }
            }
        }
        if is_full_prefix {
            score = score.saturating_mul(10000);
        }
        Ok(score)
    }

    /// Take a document and a primary key and insert into a collection.
    pub fn insert(&self, tx: &impl WriteTx, doc: &T, primary_key: &[u8]) -> Result<()> {
        let key = (self.serialize)(doc);
        if self.options.unique {
            if tx.get(&self.table_name(), key.as_slice())?.is_some() {
                anyhow::bail!(
                    "Collection \"{}\" index \"{}\" cannot insert document, uniqueness constraint violated",
                    self.collection_name,
                    self.table_name()
                );
            }
            tx.insert(&self.table_name(), key.as_slice(), primary_key)?;
        } else {
            tx.insert_multimap(&self.table_name(), key.as_slice(), primary_key)?;
        }
        Ok(())
    }
}
