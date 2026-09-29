mod any;
mod write_tx;

pub use any::*;
pub use write_tx::*;

use std::borrow::Borrow;
use std::collections::HashSet;

use anyhow::Result;
use futures::StreamExt;
use futures::future::Either;

use super::*;

const DESCRIPTION_KEY: &'static [u8] = "description".as_bytes();
const MONOTONIC_ID_KEY: &'static [u8] = "monotonic".as_bytes();

pub trait QueryOperations<T: CollectionDocument>:
    ReadWriteQueryOperations<T> + WriteQueryOperations<T>
{
}

pub trait ReadWriteQueryOperations<T: CollectionDocument> {
    fn find_many(
        &self,
        query: <T as Queryable>::DocumentQuery,
    ) -> impl Future<Output = Result<impl futures::Stream<Item = Result<T>> + Send + Sync>>;
    fn find_one(
        &self,
        query: <T as Queryable>::DocumentQuery,
    ) -> impl Future<Output = Result<Option<T>>>;
    fn update(
        &self,
        query: <T as Queryable>::DocumentQuery,
        update_fn: impl Fn(&T) -> Option<T>,
    ) -> impl Future<Output = Result<()>>;
    fn delete(
        &self,
        query: <T as Queryable>::DocumentQuery,
        delete_fn: impl Fn(&T) -> bool,
    ) -> impl Future<Output = Result<()>>;
}

pub trait WriteQueryOperations<T: CollectionDocument> {
    fn update_exact(
        &self,
        query_doc: <T as Queryable>::DocumentQuery,
        update_fn: impl FnOnce(T) -> T,
    ) -> impl Future<Output = Result<Option<T>>>;
    fn delete_exact(
        &self,
        query_doc: <T as Queryable>::DocumentQuery,
    ) -> impl Future<Output = Result<Option<T>>>;
    fn find_exact(
        &self,
        query_doc: <T as Queryable>::DocumentQuery,
    ) -> impl Future<Output = Result<Option<T>>>;
}

#[derive(Debug, Clone)]
pub struct Collection<T: CollectionDocument, K: KV> {
    kv: K,
    name: String,
    indices: Vec<Index<T>>,
    primary_index: PrimaryIndex<T>,
}

impl<T: CollectionDocument + 'static, K: KV> Collection<T, K> {
    pub async fn new(
        kv: K,
        name: String,
        primary_index: PrimaryIndex<T>,
        indices: Vec<Index<T>>,
    ) -> Result<Self> {
        if name.contains(NAME_SEPARATOR) {
            anyhow::bail!(
                "collection name \"{name}\" may not contain reserved sequence: \"{NAME_SEPARATOR}\""
            )
        }
        Self {
            kv,
            name,
            primary_index,
            indices,
        }
        .migrate_if_needed()
        .await
    }

    pub fn query<'a>(&'a self) -> T::RunnableDocumentQuery<'a, Self> {
        T::RunnableDocumentQuery::new(self)
    }

    fn metatable(&self) -> String {
        <Self as AnyCollection<K>>::metatable(self)
    }

    /// Retrieve a monotonic id specific to this table.
    pub async fn monotonic_id(&self) -> Result<u64> {
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        let id = write.monotonic_id().await?;
        write_tx.commit().await?;
        Ok(id)
    }

    async fn migrate_if_needed(self) -> Result<Self> {
        log::info!("initializing collection: \"{}\"", self.name);
        if !self.indices.is_empty() {
            log::info!("non-primary indices: {}", self.indices.len());
        }
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        let metatable = self.metatable();
        // version is determined by number of migrations defined
        // this is the version of our implementation
        let doc_version = T::migrations().len() as u32;
        log::info!("document impl version: {}", doc_version);
        let existing_description =
            if let Some(item) = self.kv.get(&metatable, DESCRIPTION_KEY).await? {
                postcard::from_bytes(&item.value().await)?
            } else {
                log::info!("no metadata document found, creating...");
                // persist the names and types of fields
                let description = CollectionDescription {
                    version: doc_version,
                    fields: T::owned_field_types().clone(),
                    primary_key_fields: self.primary_index.fields().clone(),
                    indices: Default::default(),
                };
                write_tx
                    .insert(
                        &metatable,
                        DESCRIPTION_KEY,
                        &postcard::to_stdvec(&description)?,
                    )
                    .await?;
                description
            };
        assert!(
            existing_description.version <= doc_version,
            "attempting to open database file from newer version of schema!"
        );
        // run migrations if needed
        if existing_description.version < doc_version {
            log::info!(
                "\"{}\" stored document version {}, impl version {doc_version}",
                self.name,
                existing_description.version
            );
            // use a read transaction to get a consistent view
            // of the collection during migration
            let read_tx = self.kv.read_tx().await?;
            log::info!(
                "migrating {} documents...",
                read_tx.count(&self.name).await?
            );
            // wipe the collection and indices for the current transaction
            write.clear().await?;
            assert_eq!(write_tx.count(&self.name).await?, 0);
            for index in &self.indices {
                assert_eq!(write_tx.count(index.kv_table_name()).await?, 0);
            }
            let value_fields = existing_description
                .fields
                .iter()
                .filter_map(|(name, _)| {
                    if existing_description
                        .primary_key_fields
                        .iter()
                        .find(|v| &v.0 == name)
                        .is_none()
                    {
                        Some(name.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let out = read_tx.range(&self.name, ..).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(item) = stream.next().await.transpose()? {
                let mut key = item.key();
                let mut value = item.value().await;
                let mut values = Vec::with_capacity(existing_description.fields.len());
                for (name, stats) in existing_description.primary_key_fields.iter() {
                    let len = if let Some(len) = stats.fixed_width {
                        len as usize
                    } else {
                        let (len, rem) = postcard::take_from_bytes::<'_, u64>(value)?;
                        value = rem;
                        len as usize
                    };
                    let (v, rem) = key.split_at(len);
                    if rem.is_empty() {
                        key = rem;
                    } else {
                        let (sep, rem) = rem.split_at(1);
                        assert_eq!(sep[0], 0);
                        key = rem;
                    }
                    values.push((name.to_string(), v.to_vec()));
                }

                let mut state = (values, value_fields.clone(), value.to_vec());
                for i in existing_description.version..doc_version {
                    state = T::migrations()[i as usize](state.0, state.1, state.2)?;
                }
                if self.primary_index.fields().len() != state.0.len() {
                    anyhow::bail!("key field length mismatch")
                }
                let key_fields = self
                    .primary_index
                    .fields()
                    .iter()
                    .zip(state.0.into_iter())
                    .map(|(i, v)| {
                        if i.0 != v.0 {
                            anyhow::bail!(
                                "primary key field name mismatch. expected \"{}\", got \"{}\"",
                                i.0,
                                v.0
                            )
                        }
                        Ok(v.1)
                    })
                    .collect::<Result<LexicographicKey>>()?
                    .into_vec();
                let new_doc = self.primary_index.deserialize(&key_fields, &state.2);
                // TODO: perhaps don't fail if indices conflict in production
                // fall back to non-accelerated queries to prevent application crashes
                // always fail fast during development
                write.insert(new_doc).await?;
            }
            // update the metadata document
            write_tx
                .insert(
                    &metatable,
                    DESCRIPTION_KEY,
                    &postcard::to_stdvec(&CollectionDescription {
                        version: doc_version,
                        fields: T::owned_field_types().clone(),
                        primary_key_fields: self.primary_index.fields().clone(),
                        indices: self
                            .indices
                            .iter()
                            .map(|index| index.description())
                            .collect(),
                    })?,
                )
                .await?;
            log::info!("collection \"{}\" migration complete!", self.name);
        } else if T::owned_field_types() != existing_description.fields {
            anyhow::bail!("collection \"{}\" has inconsistent stored data!", self.name)
        } else {
            let mut matched_indices = HashSet::<String>::default();
            for index in existing_description.indices.iter() {
                if let Some(v) = self.indices.iter().find(|v| v.matches(index)) {
                    if !matched_indices.insert(v.kv_table_name().into()) {
                        anyhow::bail!("duplicate index name detected: \"{}\"", index.table_name)
                    }
                    // otherwise the index is unchanged
                } else {
                    log::info!("dropping index: \"{}\"", index.table_name);
                    // we dropped an index from the schema
                    if index.options.unique {
                        write_tx.clear(&index.table_name).await?;
                    } else {
                        write_tx.clear_multimap(&index.table_name).await?;
                    }
                }
            }
            let mut indices_needing_rebuild = Vec::default();
            for index in self.indices.iter() {
                if !matched_indices.contains(index.kv_table_name()) {
                    // needs to be rebuilt
                    indices_needing_rebuild.push(index);
                }
            }
            if !indices_needing_rebuild.is_empty() {
                log::info!("collection \"{}\" needs index rebuild", self.name);
                for index in indices_needing_rebuild.iter() {
                    index.clear(&write_tx).await?;
                }
                let read_tx = self.kv.read_tx().await?;
                let out = read_tx.range(&self.name, ..).await?;
                let mut stream = std::pin::pin!(out);
                while let Some(item) = stream.next().await.transpose()? {
                    let doc = self
                        .primary_index
                        .deserialize(item.key(), item.value().await);
                    for index in indices_needing_rebuild.iter() {
                        index.insert(&write_tx, &doc, item.key()).await?;
                    }
                }
            }
            write_tx
                .insert(
                    &metatable,
                    DESCRIPTION_KEY,
                    &postcard::to_stdvec(&CollectionDescription {
                        version: doc_version,
                        fields: T::owned_field_types(),
                        primary_key_fields: self.primary_index.fields().clone(),
                        indices: self
                            .indices
                            .iter()
                            .map(|index| index.description())
                            .collect(),
                    })?,
                )
                .await?;
        }
        // commit the transaction
        write_tx.commit().await?;
        Ok(self)
    }

    /// Get a reference to the backing KV.
    pub fn kv(&self) -> K {
        self.kv.clone()
    }

    /// The name of the collection. This is the name of the table that will be used in the
    /// underlying KV.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the number of documents in the collection.
    pub async fn count(&self) -> Result<u64> {
        self.kv.count(&self.name).await
    }

    /// Batch insert many documents into a collection in a single
    /// atomic transaction.
    pub async fn insert_many<B>(&self, document_iter: impl Iterator<Item = B>) -> Result<()>
    where
        B: Borrow<T> + Send + Sync,
    {
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        for item in document_iter {
            write.insert(item).await?;
        }
        write_tx.commit().await?;
        Ok(())
    }

    /// Insert a document into a collection. All relevant indices will be updated.
    pub async fn insert(&self, document: impl Borrow<T> + Send + Sync) -> Result<()> {
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        write.insert(document).await?;
        write_tx.commit().await?;
        Ok(())
    }

    /// Insert or replace a document in the collection.
    pub async fn upsert(&self, document: impl Borrow<T> + Send + Sync) -> Result<()> {
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        write.upsert(document).await?;
        write_tx.commit().await?;
        Ok(())
    }

    /// Select the index which matches a query best.
    /// A None value indicates retrieval by primary key.
    fn select_index(
        &self,
        query_fields: &Vec<(&str, QueryParam<Vec<u8>>)>,
    ) -> Result<Option<&Index<T>>> {
        let (mut compat, mut out_index): (IndexQueryCompatibility, Option<&Index<T>>);
        compat = IndexQueryCompatibility::new(&self.primary_index.fields(), query_fields)?;
        out_index = None;

        for index in self.indices.iter() {
            if compat == IndexQueryCompatibility::CompleteUnique {
                return Ok(out_index);
            }
            let index_compat = IndexQueryCompatibility::new(index.field_names(), query_fields)?;
            if index_compat > compat {
                compat = index_compat;
                out_index = Some(index);
            }
        }
        Ok(out_index)
    }

    pub(crate) async fn query_primary_key<'a>(
        &'a self,
        tx: impl ReadOperations + RangeReadOperations + 'a,
        query: <T as Queryable>::DocumentQuery,
        scan_range: GeneralRange<Vec<u8>>,
    ) -> Result<impl futures::Stream<Item = Result<T>> + 'a> {
        Ok(async_stream::try_stream! {
            let out = tx.range(&self.name, scan_range).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(item) = stream.next().await.transpose()? {
                let doc = self.primary_index.deserialize(item.key(), item.value().await);
                if doc.matches(&query) {
                    yield doc;
                }
            }
        })
    }

    pub async fn clear(&self) -> Result<()> {
        let write_tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &write_tx);
        write.clear().await?;
        write_tx.commit().await
    }
}

impl<T: CollectionDocument + 'static, K: KV> QueryOperations<T> for Collection<T, K> {}

impl<T: CollectionDocument + 'static, K: KV> WriteQueryOperations<T> for Collection<T, K> {
    async fn find_exact(&self, query_doc: <T as Queryable>::DocumentQuery) -> Result<Option<T>> {
        let query: Query = query_doc.clone().into();
        if let Some(key) = self.primary_index.key_maybe(&query) {
            let Some(item) = self.kv.get(&self.name, &key).await? else {
                return Ok(None);
            };
            let doc = self
                .primary_index
                .deserialize(item.key(), item.value().await);
            return Ok(if doc.matches(&query_doc) {
                Some(doc)
            } else {
                None
            });
        }
        for index in self.indices.iter() {
            if !index.options().unique || index.key_maybe(&query).is_none() {
                continue;
            }
            if let Some(doc) = index
                .get_exact(&self.primary_index, &self.kv.read_tx().await?, &query)
                .await?
                && doc.matches(&query_doc)
            {
                return Ok(Some(doc));
            } else {
                return Ok(None);
            }
        }
        anyhow::bail!(
            "Query did not fully specify a primary key or unique index key: {:?}",
            query_doc
        )
    }

    async fn update_exact(
        &self,
        query_doc: <T as Queryable>::DocumentQuery,
        update_fn: impl FnOnce(T) -> T,
    ) -> Result<Option<T>> {
        let tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &tx);
        let out = write.update_exact(query_doc, update_fn).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn delete_exact(&self, query_doc: <T as Queryable>::DocumentQuery) -> Result<Option<T>> {
        let tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &tx);
        let out = write.delete_exact(query_doc).await?;
        tx.commit().await?;
        Ok(out)
    }
}

impl<T: CollectionDocument + 'static, K: KV> ReadWriteQueryOperations<T> for Collection<T, K> {
    async fn find_many(
        &self,
        query: <T as Queryable>::DocumentQuery,
    ) -> Result<impl futures::Stream<Item = Result<T>> + Send + Sync> {
        let query_erased = query.clone().into();
        let index_maybe = self.select_index(&query_erased.fields)?;
        let tx = self.kv.read_tx().await?;
        if let Some(index) = index_maybe {
            let scan_range = query_erased.compute_scan_range(index.field_names());
            Ok(Either::Left(
                index
                    .query(self.primary_index.clone(), tx, query, scan_range)
                    .await?,
            ))
        } else {
            let scan_range = query_erased.compute_scan_range(self.primary_index.fields());
            Ok(Either::Right(
                self.query_primary_key(tx, query, scan_range).await?,
            ))
        }
    }

    async fn find_one(&self, query: <T as Queryable>::DocumentQuery) -> Result<Option<T>> {
        let out = self.find_many(query).await?;
        futures::pin_mut!(out);
        out.next().await.transpose()
    }

    /// Given a query, repeatedly call a closure with documents matching the
    /// query. If the closure returns `Some()` the original document is replaced
    /// and all indices are updated. If the closure returns `None`, no operation occurs.
    async fn update(
        &self,
        query: <T as Queryable>::DocumentQuery,
        update_fn: impl Fn(&T) -> Option<T>,
    ) -> Result<()> {
        let tx = self.kv.write_tx().await?;
        let write = CollectionWrite::new(&self, &tx);
        let out = self.find_many(query).await?;
        let mut stream = std::pin::pin!(out);
        while let Some(doc) = stream.next().await.transpose()? {
            if let Some(updated) = update_fn(&doc) {
                let primary_key = self.primary_index.serialize_key(&doc);
                tx.remove(&self.name, &primary_key).await?;
                for index in self.indices.iter() {
                    index.delete(&tx, &doc, &primary_key).await?;
                }
                write.insert(&updated).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn delete(
        &self,
        query: <T as Queryable>::DocumentQuery,
        delete_fn: impl Fn(&T) -> bool,
    ) -> Result<()> {
        let tx = self.kv.write_tx().await?;
        let out = self.find_many(query).await?;
        let mut stream = std::pin::pin!(out);
        while let Some(doc) = stream.next().await.transpose()? {
            if delete_fn(&doc) {
                let primary_key = self.primary_index.serialize_key(&doc);
                tx.remove(&self.name, &primary_key).await?;
                for index in self.indices.iter() {
                    index.delete(&tx, &doc, &primary_key).await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
