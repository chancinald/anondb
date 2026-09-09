use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone)]
pub struct Collection<T, K: KV>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable,
{
    kv: K,
    name: String,
    primary_key_index: Index<T>,
    indices: Vec<Index<T>>,
}

impl<T, K: KV> Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable,
{
    pub fn new(kv: K, name: String, primary_key_index: Index<T>, indices: Vec<Index<T>>) -> Self {
        Self {
            kv,
            name,
            primary_key_index,
            indices,
        }
    }

    pub fn primary_key_index(&self) -> &Index<T> {
        &self.primary_key_index
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
        self.kv().count(self.name()).await
    }

    /// Insert a document into a collection. All relevant indices will be updated.
    pub async fn insert(&self, document: &T) -> Result<()> {
        let tx = self.kv().write_tx().await?;
        let primary_key = (self.primary_key_index.serialize_key)(document);
        self.primary_key_index()
            .insert(&tx, document, &primary_key)
            .await?;
        for index in &self.indices {
            index.insert(&tx, document, &primary_key).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Clear all indices for this collection and completely rebuild them. This operation is `O(N)`
    /// over the number of documents in the collection.
    pub async fn rebuild_indices(&self) -> Result<()> {
        // first empty all index collections
        // let tx = self.kv().write_tx().await?;
        // for index in &self.indices {
        //     tx.clear(&index.table_name()).await?;
        // }
        // tx.commit().await?;

        // then iterate over all documents and construct indices
        // self.kv().scan(self.name(), |primary_key, val| {
        //     let data = rmp_serde::from_slice(val)?;
        //     for index in &self.indices {
        //         let key = (index.serialize)(&data);
        //         // we insert the document primary key as the value and the indexed bytes as the
        //         // key to get lexicographic iteration
        //         self.kv()
        //             .insert(&index.table_name(), key.as_slice(), primary_key)
        //             .await?;
        //     }
        //     return Ok(true);
        // })?;
        Ok(())
    }

    /// Select the index which matches a query best.
    fn select_index(&self, query_fields: &Vec<(&str, Param)>) -> Result<&Index<T>> {
        let (mut compat, mut out_index): (IndexQueryCompatibility, &Index<T>);
        compat = IndexQueryCompatibility::new(&self.primary_key_index, query_fields)?;
        out_index = &self.primary_key_index;

        for index in self.indices.iter() {
            if compat == IndexQueryCompatibility::CompleteUnique {
                return Ok(out_index);
            }
            let index_compat = IndexQueryCompatibility::new(out_index, query_fields)?;
            if index_compat > compat {
                compat = index_compat;
                out_index = index;
            }
        }
        Ok(out_index)
    }

    pub async fn find_many(
        &self,
        query: T::DocumentQuery,
    ) -> Result<impl futures::Stream<Item = Result<T>>> {
        let fields = query.fields();
        let index = self.select_index(&fields)?;
        let tx = self.kv().read_tx().await?;
        index.query(tx, query, fields).await
    }

    pub async fn find_one(&self, query: T::DocumentQuery) -> Result<Option<T>> {
        let fields = query.fields();
        let index = self.select_index(&fields)?;
        let tx = self.kv().read_tx().await?;
        let out = index.query(tx, query, fields).await?;
        futures::pin_mut!(out);
        out.next().await.transpose()
    }
}
