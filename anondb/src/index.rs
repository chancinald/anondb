use std::borrow::Cow;
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
    pub unique: bool, // only allow 1 unique combination of each field in the index
    pub primary: bool, // does the index store the full document in a table matching the collection
                      // name
}

// TODO: explicitly check and disallow duplicate field names
#[derive(Debug, Clone, PartialEq)]
pub struct Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    /// The name of the collection the index belongs to.
    pub collection_name: String,
    /// The field names of the document type along with the byte length (if constant)
    pub field_names: Vec<(String, LexStats)>,
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
        if self.options.primary {
            self.collection_name.to_string()
        } else {
            format!(
                "{}_{}{}",
                self.collection_name,
                self.field_names
                    .iter()
                    .map(|(name, _)| name.to_string())
                    .collect::<Vec<_>>()
                    .join("_"),
                if self.options.unique { "_unique" } else { "" }
            )
        }
    }

    /// TODO: allow variable length serialization only as the final element in an index
    /// Accept a set of field keys as lexicographically serialized bytes
    pub fn query<'tx>(
        &self,
        tx: &'tx impl ReadOperations,
        query: &T::DocumentQuery,
        index_fields: &HashMap<String, Param>,
    ) -> Result<impl Iterator<Item = T>> {
        let mut min_key = LexicographicKey::default();
        let mut max_key = LexicographicKey::default();
        let mut min_bound: Bound<Vec<u8>> = Bound::Unbounded;
        let mut max_bound: Bound<Vec<u8>> = Bound::Unbounded;
        for (name, lex_stats) in &self.field_names {
            if let Some(query_param) = index_fields.get(name) {
                match query_param {
                    Param::Eq(v) => {
                        min_key.append_key_slice(v);
                        max_key.append_key_slice(v);
                        min_bound = Bound::Included(min_key.to_vec());
                        max_bound = Bound::Included({
                            let mut v = max_key.clone();
                            v.append_upper_inclusive_byte();
                            v.take()
                        });
                    }
                    Param::In(_v) => {
                        unimplemented!()
                    }
                    Param::Nin(_) | Param::Neq(_) => {
                        break;
                    }
                    Param::Range(v) => {
                        match v.start_bound() {
                            Bound::Unbounded => {}
                            Bound::Included(v) | Bound::Excluded(v) => {
                                // we always treat it as included to account for earlier fields
                                // that may exist in the key
                                min_key.append_key_slice(&v);
                                min_bound = Bound::Included(min_key.take());
                            }
                        }
                        match v.end_bound() {
                            Bound::Unbounded => {
                                max_bound = Bound::Unbounded;
                            }
                            Bound::Included(v) | Bound::Excluded(v) => {
                                max_key.append_key_slice(&v);
                                max_key.append_upper_inclusive_byte();
                                max_bound = Bound::Included(max_key.take());
                            }
                        }
                        break;
                    }
                }
            } else {
                // the query isn't using this field of the index. If this field is constant width
                // we can continue attempting to use the index.
                if let Some(width) = lex_stats.fixed_width {
                    let min = vec![0u8; width as usize];
                    let max = vec![u8::MAX; width as usize];
                    min_key.append_key_slice(&min);
                    max_key.append_key_slice(&max);
                    min_bound = Bound::Included(min_key.to_vec());
                    max_bound = Bound::Included({
                        let mut v = max_key.clone();
                        v.append_upper_inclusive_byte();
                        v.take()
                    });
                    continue;
                }

                // otherwise we have to halt and begin scanning
                break;
            }
        }
        let scan_range = GeneralRange(min_bound, max_bound);
        println!("{:?}", scan_range);
        let table_name = self.table_name();
        let docs = if self.options.unique {
            tx.range_buffered(&table_name, scan_range.as_slice(), |_k, v, _done| {
                // v represents the primary key, we'll load the document and check it against the
                // selector
                let doc_bytes = if self.options.primary {
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
        for (i, (name, lex_stats)) in self.field_names.iter().enumerate() {
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
            if lex_stats.fixed_width.is_none() {
                if !is_last_field {
                    is_full_prefix = false;
                }
                break;
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
        let table_name = self.table_name();
        let bytes = if self.options.primary {
            Cow::from(rmp_serde::to_vec_named(doc)?)
        } else {
            Cow::from(primary_key)
        };
        if self.options.unique {
            if tx.get(&table_name, key.as_slice())?.is_some() {
                anyhow::bail!(
                    "Collection \"{}\" index \"{}\" cannot insert document, uniqueness constraint violated",
                    self.collection_name,
                    table_name
                );
            }
            tx.insert(&table_name, key.as_slice(), &bytes)?;
        } else {
            tx.insert_multimap(&table_name, key.as_slice(), &bytes)?;
        }
        Ok(())
    }
}
