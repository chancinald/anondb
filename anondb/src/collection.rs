use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::*;
use anondb_kv::*;

#[derive(Debug)]
pub struct Collection<T, K: KV>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable,
{
    kv: K,
    name: String,
    /// Extractor function to get a primary key from an instance of T
    primary_key_index: Index<T>,
}

impl<T, K: KV> Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable,
{
    pub fn new(kv: K, name: String, primary_key_index: Index<T>) -> Self {
        Self {
            kv,
            name,
            primary_key_index,
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
        let primary_key = (self.primary_key_index.serialize)(document);

        self.primary_key_index()
            .insert(&tx, document, &primary_key)
            .await?;
        // for index in &self.indices {
        //     index.insert(&tx, document, &primary_key)?;
        // }
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

    // pub async fn find_many(
    //     &self,
    //     query: T::DocumentQuery,
    // ) -> Result<impl Iterator<Item = T> + DoubleEndedIterator> {
    //     let index_fields = self.extract_index_fields(&query);
    //
    //     let mut scores = BTreeMap::default();
    //     let primary_index_score = self
    //         .primary_key_index()
    //         .query_compat(&query, &index_fields)?;
    //     scores.insert(primary_index_score, self.primary_key_index());
    //
    //     for index in self.indices() {
    //         let score = index.query_compat(&query, &index_fields)?;
    //         scores.insert(score, index.clone());
    //     }
    //     let (score, best_index) = scores
    //         .last_key_value()
    //         .map(|(k, v)| (*k, v.clone()))
    //         .ok_or(anyhow::anyhow!("no index found"))?;
    //     println!(
    //         "using index \"{}\" score: {}",
    //         best_index.table_name(),
    //         score
    //     );
    //     let tx = self.kv().read_tx().await?;
    //     Ok(best_index
    //         .query(&tx, &query, &index_fields)?
    //         .collect::<Vec<_>>()
    //         .into_iter())
    // }
    //
    // pub async fn find_one(&self, query: T::DocumentQuery) -> Result<Option<T>> {
    //     let index_fields = self.extract_index_fields(&query);
    //
    //     let mut scores = BTreeMap::default();
    //     let primary_index_score = self
    //         .primary_key_index()
    //         .query_compat(&query, &index_fields)?;
    //     scores.insert(primary_index_score, self.primary_key_index().clone());
    //     for index in self.indices() {
    //         let score = index.query_compat(&query, &index_fields)?;
    //         scores.insert(score, index.clone());
    //     }
    //     if let Some((score, best_index)) = scores.last_key_value() {
    //         println!(
    //             "using index \"{}\" score: {}",
    //             best_index.table_name(),
    //             score
    //         );
    //         let tx = self.kv().read_tx().await?;
    //         let mut out = best_index.query(&tx, &query, &index_fields)?;
    //         Ok(out.next())
    //     } else {
    //         unreachable!("no index found!");
    //     }
    // }
}
