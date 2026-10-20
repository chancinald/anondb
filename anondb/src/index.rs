use std::borrow::Borrow;
use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;

use super::*;

pub trait AnyIndex {
    fn collection_name(&self) -> &str;
    fn field_names(&self) -> &Vec<(String, OwnedLexStats)>;
    fn options(&self) -> &IndexOptions;
    fn kv_table_name(&self) -> &str;
    fn parse_index_key(&self, key: &[u8]) -> Result<Vec<(String, String, Box<dyn ValidDBType>)>>;
}

impl<T: CollectionDocument> AnyIndex for Index<T>
where
    T: Queryable + CollectionDocument,
{
    fn collection_name(&self) -> &str {
        &self.collection_name
    }
    fn field_names(&self) -> &Vec<(String, OwnedLexStats)> {
        &self.field_names
    }
    fn options(&self) -> &IndexOptions {
        &self.options
    }
    fn kv_table_name(&self) -> &str {
        &self.kv_table_name
    }
    fn parse_index_key(&self, key: &[u8]) -> Result<Vec<(String, String, Box<dyn ValidDBType>)>> {
        let maybe_doc = (self.deserialize_key)(key)?;
        let out = maybe_doc.serialize_remaining_to_vec();
        assert_eq!(out.len(), self.field_names.len());
        for (v, x) in out.iter().zip(self.field_names.iter()) {
            assert_eq!(v.0, x.0);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct IndexOptions {
    /// only allow 1 unique combination of each field in the index
    pub unique: bool,
}

// TODO: explicitly check and disallow duplicate field names
#[derive(Debug, Clone)]
pub struct Index<T>
where
    T: Queryable + CollectionDocument,
{
    /// The name of the database collection the index belongs to.
    collection_name: String,
    /// The field names of the document type along with the byte length (if constant)
    pub(crate) field_names: Vec<(String, OwnedLexStats)>,
    /// Take a document of type `T` and serialize it into a lexicographically sortable key
    pub(crate) serialize_key: Arc<fn(&T) -> Result<Vec<u8>>>,
    pub(crate) deserialize_key:
        Arc<fn(&[u8]) -> Result<<T as CollectionDocument>::MaybeStruct<'_>>>,
    /// Options for the index
    options: IndexOptions,
    kv_table_name: String,
}

impl<T> Index<T>
where
    T: Queryable + CollectionDocument,
{
    pub fn new(
        collection_name: String,
        field_names: Vec<(String, OwnedLexStats)>,
        serialize: fn(&T) -> Result<Vec<u8>>,
        deserialize: fn(&[u8]) -> Result<<T as CollectionDocument>::MaybeStruct<'_>>,
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
            serialize_key: Arc::new(serialize),
            deserialize_key: Arc::new(deserialize),
            options,
            kv_table_name,
        }
    }

    pub(crate) fn field_names(&self) -> &Vec<(String, OwnedLexStats)> {
        &self.field_names
    }

    pub(crate) fn kv_table_name(&self) -> &str {
        &self.kv_table_name
    }

    pub(crate) fn description(&self) -> IndexDescription {
        IndexDescription {
            field_names: self
                .field_names
                .iter()
                .map(|(name, _)| name.to_string())
                .collect(),
            options: self.options.clone(),
            table_name: self.kv_table_name.clone(),
        }
    }

    pub(crate) fn matches(&self, description: &IndexDescription) -> bool {
        self.field_names
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>()
            == description.field_names
            && self.options == description.options
            && self.kv_table_name == description.table_name
    }

    pub(crate) async fn clear<'a>(
        &'a self,
        tx: &'a (impl ReadOperations + WriteOperations),
    ) -> Result<()> {
        if self.options.unique {
            tx.clear(&self.kv_table_name).await?;
        } else {
            tx.clear_multimap(&self.kv_table_name).await?;
        }
        Ok(())
    }

    /// TODO: allow variable length serialization only as the final element in an index
    /// Accept a set of field keys as lexicographically serialized bytes
    pub(crate) async fn query<'a>(
        &'a self,
        primary_index: PrimaryIndex<T>,
        tx: impl ReadOperations + RangeReadOperations + 'static,
        query: <T as Queryable>::DocumentQuery,
        scan_range: GeneralRange<Vec<u8>>,
    ) -> Result<impl futures::Stream<Item = Result<T>> + Send + Sync + 'a> {
        Ok(async_stream::try_stream! {
            if self.options.unique {
                let out = tx.range(&self.kv_table_name, scan_range).await?;
                let mut stream = std::pin::pin!(out);
                while let Some(item) = stream.next().await.transpose()? {
                    let entry = tx.get(&self.collection_name, item.value().await)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!(
                            "Index \"{}\" referencing primary key that does not exist!",
                            self.kv_table_name
                        ))?;
                    let doc = primary_index.deserialize(entry.key(), entry.value().await)?;
                    if doc.matches(&query)? {
                        yield doc;
                    }
                }
            } else {
                // non-unique indices map each key to the primary keys of every matching document
                let out = tx.range_multimap(&self.kv_table_name, scan_range).await?;
                let mut stream = std::pin::pin!(out);
                while let Some(item) = stream.next().await.transpose()? {
                    let entry = tx.get(&self.collection_name, item.value().await)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!(
                            "Index \"{}\" referencing primary key that does not exist!",
                            self.kv_table_name
                        ))?;
                    let doc = primary_index.deserialize(entry.key(), entry.value().await)?;
                    if doc.matches(&query)? {
                        yield doc;
                    }
                }
            }
        })
    }

    pub(crate) async fn delete(
        &self,
        tx: &impl WriteTx,
        doc: impl Borrow<T> + Send + Sync,
        primary_key: &[u8],
    ) -> Result<()> {
        let key = (self.serialize_key)(doc.borrow())?;
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
        doc: impl Borrow<T> + Send + Sync,
        primary_key: impl AsRef<[u8]> + Send + Sync,
    ) -> Result<()> {
        let key = (self.serialize_key)(doc.borrow())?;
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

    pub async fn get_exact(
        &self,
        primary_index: &PrimaryIndex<T>,
        tx: &impl ReadOperations,
        query: &Query,
    ) -> Result<Option<T>> {
        if !self.options.unique {
            anyhow::bail!("Cannot get exact document in non-unique index")
        }
        let Some(key) = self.key_maybe(query) else {
            anyhow::bail!(
                "Index::get_exact must be called with a query fully specifying a document"
            )
        };
        let Some(val) = tx.get(&self.kv_table_name, &key).await? else {
            return Ok(None);
        };
        let entry = tx
            .get(&self.collection_name, val.value().await)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Index \"{}\" referencing primary key that does not exist!",
                    self.kv_table_name
                )
            })?;
        Ok(Some(
            primary_index.deserialize(entry.key(), entry.value().await)?,
        ))
    }

    /// Try to serialize a query into an exact document in the index,
    /// if possible.
    pub fn key_maybe(&self, query: &Query) -> Option<Vec<u8>> {
        if query.fields.len() < self.field_names.len() {
            return None;
        }
        let mut key = LexicographicKey::default();
        for (name, _) in self.field_names.iter() {
            if let Some(bytes) = query.fields.iter().find_map(|(qname, param)| match param {
                QueryParam::Eq(bytes) => {
                    if *qname == name.as_str() {
                        Some(bytes)
                    } else {
                        None
                    }
                }
                _ => None,
            }) {
                key.append_key_slice(bytes);
            } else {
                return None;
            }
        }
        Some(key.into_vec())
    }
}
