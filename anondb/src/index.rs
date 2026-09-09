use std::ops::Bound;
use std::ops::RangeBounds;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct IndexOptions {
    /// only allow 1 unique combination of each field in the index
    pub unique: bool,
    /// does the index store the full document, or a reference to the primary key
    pub primary: bool,
}

// TODO: explicitly check and disallow duplicate field names
#[derive(Debug, Clone)]
pub struct Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    /// The name of the collection the index belongs to.
    collection_name: String,
    /// The field names of the document type along with the byte length (if constant)
    field_names: Vec<(String, LexStats)>,
    /// Take a document of type `T` and serialize it into a lexicographically sortable key
    pub(crate) serialize_key: Box<fn(&T) -> Vec<u8>>,
    /// Options for the index
    options: IndexOptions,
    kv_table_name: String,
}

impl<T> Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    pub fn new(
        collection_name: String,
        field_names: Vec<(String, LexStats)>,
        serialize: fn(&T) -> Vec<u8>,
        options: IndexOptions,
    ) -> Self {
        let kv_table_name = if options.primary {
            collection_name.clone()
        } else {
            format!(
                "{}_{}{}",
                collection_name,
                field_names
                    .iter()
                    .map(|(name, _)| name.to_string())
                    .collect::<Vec<_>>()
                    .join("_"),
                if options.unique { "_unique" } else { "" }
            )
        };
        Self {
            collection_name,
            field_names,
            serialize_key: Box::new(serialize),
            options,
            kv_table_name,
        }
    }

    pub(crate) fn field_names(&self) -> &Vec<(String, LexStats)> {
        &self.field_names
    }

    /// TODO: allow variable length serialization only as the final element in an index
    /// Accept a set of field keys as lexicographically serialized bytes
    pub async fn query<'a>(
        &'a self,
        tx: impl ReadOperations + RangeReadOperation + 'a,
        query: <T as Queryable>::DocumentQuery,
        index_fields: Vec<(&str, Param)>,
    ) -> Result<impl futures::Stream<Item = Result<T>> + 'a> {
        let mut min_key = LexicographicKey::default();
        let mut max_key = LexicographicKey::default();
        let mut min_bound: Bound<Vec<u8>> = Bound::Unbounded;
        let mut max_bound: Bound<Vec<u8>> = Bound::Unbounded;
        for (name, lex_stats) in &self.field_names {
            let Some((_, query_param)) = index_fields.iter().find(|(pname, _)| pname == name)
            else {
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
            };
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
        }
        Ok(async_stream::try_stream! {
            let scan_range = GeneralRange(min_bound, max_bound);
            if self.options.unique || self.options.primary {
                for v in tx.range(&self.kv_table_name, scan_range).await? {
                    let item = v?;
                    let doc = if self.options.primary {
                        rmp_serde::from_slice::<T>(item.value().await)?
                    } else {
                        let entry = tx.get(&self.collection_name, item.value().await)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!(
                                "Index \"{}\" referencing primary key that does not exist!",
                                self.kv_table_name
                            ))?;
                        rmp_serde::from_slice::<T>(entry.value().await)?
                    };
                    if doc.matches(&query) {
                        yield doc;
                    }
                }
            } else {
                unimplemented!()
                // tx.range_buffered_multimap(&table_name, scan_range.as_slice(), |_k, v, _done| {
                //     // multimap index never stores full documents, always load from the primary table
                //     let doc_bytes = tx.get(&self.collection_name, v)?.ok_or_else(|| {
                //         anyhow::anyhow!(
                //             "Index \"{table_name}\" referencing primary key that does not exist!"
                //         )
                //     })?;
                //     // parse the bytes
                //     let doc = rmp_serde::from_slice::<T>(&doc_bytes)?;
                //     if doc.matches(query) {
                //         Ok(Some(doc))
                //     } else {
                //         Ok(None)
                //     }
                // })?
            }
        })
    }

    /// Take a document and a primary key and insert into a collection.
    pub(crate) async fn insert(
        &self,
        tx: &impl WriteTx,
        doc: &T,
        primary_key: &[u8],
    ) -> Result<()> {
        let key = (self.serialize_key)(doc);
        if self.options.primary {
            if tx.get(&self.kv_table_name, key.as_slice()).await?.is_some() {
                anyhow::bail!(
                    "Collection \"{}\" primary index cannot insert document, uniqueness constraint violated",
                    self.collection_name,
                );
            }
            tx.insert(
                &self.kv_table_name,
                key.as_slice(),
                &rmp_serde::to_vec_named(doc)?,
            )
            .await?;
        } else if self.options.unique {
            if tx.get(&self.kv_table_name, key.as_slice()).await?.is_some() {
                anyhow::bail!(
                    "Collection \"{}\" index \"{}\" cannot insert document, uniqueness constraint violated",
                    self.collection_name,
                    self.kv_table_name
                );
            }
            tx.insert(&self.kv_table_name, key.as_slice(), primary_key)
                .await?;
        } else {
            unimplemented!()
            // tx.insert_multimap(&table_name, key.as_slice(), primary_key)?;
        }
        Ok(())
    }
}
