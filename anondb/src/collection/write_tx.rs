use super::*;

#[derive(Debug, Clone)]
pub struct CollectionWrite<'a, T: AutoDocumentTypes, K: KV>
where
    T: 'static + Queryable + AnyDocument + ParseableDocument + Migrateable,
{
    pub(crate) tx: &'a K::WriteTransaction,
    collection: &'a Collection<T, K>,
}

impl<'a, T: AutoDocumentTypes, K: KV> CollectionWrite<'a, T, K>
where
    T: 'static + Queryable + AnyDocument + ParseableDocument + Migrateable,
{
    pub fn new(collection: &'a Collection<T, K>, tx: &'a K::WriteTransaction) -> Self {
        Self { tx, collection }
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
            .serialize_key_doc(document.borrow());
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
            .serialize_key(document.borrow());
        if let Some(out) = self.tx.remove(&self.collection.name, &primary_key).await? {
            let doc = self
                .collection
                .primary_index
                .deserialize(&primary_key, &out);
            for index in self.collection.indices.iter() {
                index.delete(self.tx, &doc, &primary_key).await?;
            }
        }
        self.insert(document).await?;
        Ok(())
    }

    /// Given a query, repeatedly call a closure with documents matching the
    /// query. If the closure returns `Some()` the original document is replaced
    /// and all indices are updated. If the closure returns `None`, no operation occurs.
    pub async fn update(
        &self,
        query: <T as Queryable>::DocumentQuery,
        update_fn: impl Fn(&T) -> Option<T>,
    ) -> Result<()> {
        let out = self.collection.find_many(query).await?;
        let mut stream = std::pin::pin!(out);
        while let Some(doc) = stream.next().await.transpose()? {
            if let Some(updated) = update_fn(&doc) {
                let primary_key = self.collection.primary_index.serialize_key(&doc);
                self.tx.remove(&self.collection.name, &primary_key).await?;
                for index in self.collection.indices.iter() {
                    index.delete(self.tx, &doc, &primary_key).await?;
                }
                self.insert(&updated).await?;
            }
        }
        Ok(())
    }

    pub async fn delete(
        &self,
        query: <T as Queryable>::DocumentQuery,
        delete_fn: impl Fn(&T) -> bool,
    ) -> Result<()> {
        let out = self.collection.find_many(query).await?;
        let mut stream = std::pin::pin!(out);
        while let Some(doc) = stream.next().await.transpose()? {
            if delete_fn(&doc) {
                let primary_key = self.collection.primary_index.serialize_key(&doc);
                self.tx.remove(&self.collection.name, &primary_key).await?;
                for index in self.collection.indices.iter() {
                    index.delete(self.tx, &doc, &primary_key).await?;
                }
            }
        }
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
