use std::borrow::Borrow;

use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct IndexOptions {
    /// only allow 1 unique combination of each field in the index
    pub unique: bool,
}

// TODO: explicitly check and disallow duplicate field names
#[derive(Debug, Clone)]
pub struct Index<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Queryable,
{
    /// The name of the database collection the index belongs to.
    collection_name: String,
    /// The field names of the document type along with the byte length (if constant)
    pub(crate) field_names: Vec<(String, LexStats)>,
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
        assert!(!field_names.is_empty(), "attempting to create empty index");
        let kv_table_name = format!(
            "{NAME_SEPARATOR}{collection_name}{NAME_SEPARATOR}{}{}",
            field_names
                .iter()
                .map(|(name, _)| name.to_string())
                .collect::<Vec<_>>()
                .join("_"),
            if options.unique {
                format!("{NAME_SEPARATOR}unique")
            } else {
                "".into()
            }
        );
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

    pub(crate) fn kv_table_name(&self) -> &str {
        &self.kv_table_name
    }

    pub(crate) async fn clear<'a>(
        &'a self,
        tx: &'a (impl ReadOperations + WriteOperations),
    ) -> Result<()> {
        tx.clear(&self.kv_table_name).await?;
        Ok(())
    }

    /// TODO: allow variable length serialization only as the final element in an index
    /// Accept a set of field keys as lexicographically serialized bytes
    pub async fn query<'a>(
        &'a self,
        tx: impl ReadOperations + RangeReadOperations + 'a,
        query: <T as Queryable>::DocumentQuery,
        scan_range: GeneralRange<Vec<u8>>,
    ) -> Result<impl futures::Stream<Item = Result<T>> + 'a> {
        Ok(async_stream::try_stream! {
            let out = tx.range(&self.kv_table_name, scan_range).await?;
            let mut stream = std::pin::pin!(out);
            if self.options.unique {
                while let Some(item) = stream.next().await.transpose()? {
                    let entry = tx.get(&self.collection_name, item.value().await)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!(
                            "Index \"{}\" referencing primary key that does not exist!",
                            self.kv_table_name
                        ))?;
                    let doc = rmp_serde::from_slice::<T>(entry.value().await)?;
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

    pub(crate) async fn delete(
        &self,
        tx: &impl WriteTx,
        doc: impl Borrow<T>,
        primary_key: &[u8],
    ) -> Result<()> {
        let key = (self.serialize_key)(doc.borrow());
        Ok(if self.options.unique {
            let removed_maybe = tx.remove(&self.kv_table_name, key.as_slice()).await?;
            debug_assert!(removed_maybe.is_some(), "index is inconsistent");
        } else {
            tx.remove_multimap_value(&self.kv_table_name, key.as_slice(), primary_key)
                .await?;
        })
    }

    /// Take a document and a primary key and insert into a collection.
    pub(crate) async fn insert(
        &self,
        tx: &impl WriteTx,
        doc: impl Borrow<T>,
        primary_key: impl AsRef<[u8]>,
    ) -> Result<()> {
        let key = (self.serialize_key)(doc.borrow());
        Ok(if self.options.unique {
            if tx.get(&self.kv_table_name, key.as_slice()).await?.is_some() {
                anyhow::bail!(
                    "Collection \"{}\" index \"{}\" cannot insert document, uniqueness constraint violated",
                    self.collection_name,
                    self.kv_table_name
                );
            }
            tx.insert(&self.kv_table_name, key.as_slice(), primary_key.as_ref())
                .await?;
        } else {
            tx.insert_multimap(&self.kv_table_name, key.as_slice(), primary_key.as_ref())
                .await?;
        })
    }
}
