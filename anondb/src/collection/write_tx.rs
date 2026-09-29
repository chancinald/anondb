use super::*;

#[derive(Debug, Clone)]
pub struct CollectionWrite<'a, T: CollectionDocument, K: KV> {
    pub(crate) tx: &'a K::WriteTransaction,
    collection: &'a Collection<T, K>,
}

impl<'a, T: CollectionDocument + 'static, K: KV> CollectionWrite<'a, T, K> {
    pub fn new(collection: &'a Collection<T, K>, tx: &'a K::WriteTransaction) -> Self {
        Self { tx, collection }
    }

    pub fn query(&'a self) -> T::RunnableDocumentQuery<'a, Self> {
        T::RunnableDocumentQuery::new(&self)
    }

    /// Return the number of documents in the collection.
    pub async fn count(&self) -> Result<u64> {
        self.tx.count(&self.collection.name).await
    }

    /// Retrieve a monotonic id specific to this table.
    pub async fn monotonic_id(&self) -> Result<u64> {
        let metatable = self.collection.metatable();
        let id = if let Some(item) = self.tx.get(&metatable, MONOTONIC_ID_KEY).await? {
            let item_bytes = item.value().await;
            let mut bytes = [0u8; size_of::<u64>()];
            bytes.copy_from_slice(item_bytes);
            u64::from_be_bytes(bytes)
        } else {
            0u64
        };
        self.tx
            .insert(
                &metatable,
                MONOTONIC_ID_KEY,
                (id + 1).to_be_bytes().as_slice(),
            )
            .await?;
        Ok(id)
    }

    pub async fn insert(&self, document: impl Borrow<T> + Send + Sync) -> Result<()> {
        let (primary_key, doc) = self
            .collection
            .primary_index
            .serialize_key_doc(document.borrow())?;
        if self
            .tx
            .get(&self.collection.name, &primary_key)
            .await?
            .is_some()
        {
            anyhow::bail!(
                "Collection \"{}\" cannot insert document, primary key uniqueness constraint violated",
                self.collection.name,
            );
        }
        self.tx
            .insert(&self.collection.name, &primary_key, &doc)
            .await?;
        for index in self.collection.indices.iter() {
            index
                .insert(self.tx, document.borrow(), &primary_key)
                .await?;
        }
        Ok(())
    }

    /// Insert or replace a document in the collection.
    pub async fn upsert(&self, document: impl Borrow<T> + Send + Sync) -> Result<()> {
        let primary_key = self
            .collection
            .primary_index
            .serialize_key(document.borrow())?;
        if let Some(out) = self.tx.remove(&self.collection.name, &primary_key).await? {
            let doc = self
                .collection
                .primary_index
                .deserialize(&primary_key, &out)?;
            for index in self.collection.indices.iter() {
                index.delete(self.tx, &doc, &primary_key).await?;
            }
        }
        self.insert(document).await?;
        Ok(())
    }

    /// Remove all entries in the collection
    pub async fn clear(&self) -> Result<()> {
        self.tx.clear(self.collection.name()).await?;
        for index in self.collection.indices.iter() {
            index.clear(self.tx).await?;
        }
        Ok(())
    }
}

impl<'a, T: CollectionDocument + 'static, K: KV> WriteQueryOperations<T>
    for CollectionWrite<'a, T, K>
{
    async fn find_exact(&self, query_doc: <T as Queryable>::DocumentQuery) -> Result<Option<T>> {
        let query: Query = query_doc.clone().into();
        if let Some(key) = self.collection.primary_index.key_maybe(&query) {
            let Some(item) = self.tx.get(&self.collection.name, &key).await? else {
                return Ok(None);
            };
            let doc = self
                .collection
                .primary_index
                .deserialize(item.key(), item.value().await)?;
            return Ok(if doc.matches(&query_doc)? {
                Some(doc)
            } else {
                None
            });
        }
        for index in self.collection.indices.iter() {
            if !index.options().unique || index.key_maybe(&query).is_none() {
                continue;
            }
            if let Some(doc) = index
                .get_exact(&self.collection.primary_index, self.tx, &query)
                .await?
                && doc.matches(&query_doc)?
            {
                return Ok(Some(doc));
            } else {
                return Ok(None);
            }
        }
        anyhow::bail!(
            "Write transaction query did not fully specify a primary key or unique index key: {:?}",
            query_doc
        )
    }

    async fn delete_exact(&self, query_doc: <T as Queryable>::DocumentQuery) -> Result<Option<T>> {
        if let Some(doc) = self.find_exact(query_doc).await? {
            let key = self.collection.primary_index.serialize_key(&doc)?;
            self.tx.remove(self.collection.name(), &key).await?;
            for index in self.collection.indices.iter() {
                index.delete(self.tx, &doc, &key).await?;
            }
            Ok(Some(doc))
        } else {
            Ok(None)
        }
    }

    async fn update_exact(
        &self,
        query_doc: <T as Queryable>::DocumentQuery,
        update_fn: impl FnOnce(T) -> T,
    ) -> Result<bool> {
        if let Some(doc) = self.find_exact(query_doc).await? {
            let doc = update_fn(doc);
            let (key, value) = self.collection.primary_index.serialize_key_doc(&doc)?;
            self.tx.insert(self.collection.name(), &key, &value).await?;
            for index in self.collection.indices.iter() {
                index.insert(self.tx, &doc, &key).await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
