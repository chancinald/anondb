use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone)]
pub struct Collection<T, K: KV>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable + AnyDocument + Migrateable,
{
    kv: K,
    name: String,
    primary_key_index: Index<T>,
    indices: Vec<Index<T>>,
}

impl<T, K: KV> Collection<T, K>
where
    T: 'static + Serialize + for<'de> Deserialize<'de> + Queryable + AnyDocument + Migrateable,
{
    pub async fn new(
        kv: K,
        name: String,
        primary_key_index: Index<T>,
        indices: Vec<Index<T>>,
    ) -> Result<Self> {
        let tx = kv.write_tx().await?;
        let meta_table = format!("__metadata_{name}");
        let description_key = "description".as_bytes();
        // version is determined by number of migrations defined
        // this is the version of our implementation
        let doc_version = T::migrations().len() as u32;
        if let Some(item) = kv.get(&meta_table, description_key).await? {
            // description of the database data
            let description: CollectionDescription = rmp_serde::from_slice(&item.value().await)?;
            // run migrations as needed
            assert!(
                description.version <= doc_version,
                "attempting to open database file from newer version of schema!"
            );
            if description.version < doc_version {
                // run migration
                log::info!(
                    "collection \"{name}\" current version {}, impl version {doc_version}, running migration...",
                    description.version
                );
                let read_tx = kv.read_tx().await?;
                let migration_table = format!("__migration_{name}");
                // drop any in progress migration table if it exists
                tx.rename_table(&name, &migration_table).await?;
                assert_eq!(tx.count(&name).await?, 0);
                for v in read_tx.range(&migration_table, ..).await? {
                    let item = v?;
                    let mut bytes = item.value().await;
                    let values = rmpv::decode::read_value(&mut bytes)?
                        .as_array()
                        .ok_or(anyhow::anyhow!("data was not an array"))?
                        .clone();
                    assert_eq!(values.len(), description.fields.len());
                    let mut data = description
                        .fields
                        .iter()
                        .zip(values.into_iter())
                        .map(|((name, _), val)| Ok((name.clone(), val)))
                        .collect::<Result<Vec<_>>>()?;
                    for i in description.version..doc_version {
                        data = T::migrations()[i as usize](data)?;
                    }
                    let new_doc = T::from_tuple(data);
                    primary_key_index.insert(&tx, &new_doc, &[]).await?;
                }
                for index in &indices {
                    assert!(!index.is_primary(), "refusing to clear primary index");
                    // drop the indices and rebuild
                    index.clear(&tx).await?;
                }
                // remove the previous table of values
                tx.clear(&migration_table).await?;
                // update the metadata document
                tx.insert(
                    &meta_table,
                    "description".as_bytes(),
                    &rmp_serde::to_vec(&CollectionDescription {
                        version: doc_version,
                        fields: T::field_types().clone(),
                    })?,
                )
                .await?;
                log::info!("collection \"{name}\" migration complete!");
            } else if T::field_types() != &description.fields {
                anyhow::bail!("collection \"{name}\" has incorrect fields!")
            }
        } else {
            log::info!("initializing collection: {name}");
            // persist the names and types of fields
            let description = CollectionDescription {
                version: doc_version,
                fields: T::field_types().clone(),
            };
            tx.insert(
                &meta_table,
                description_key,
                &rmp_serde::to_vec(&description)?,
            )
            .await?;
        }
        // commit the transaction
        tx.commit().await?;
        Ok(Self {
            kv,
            name,
            primary_key_index,
            indices,
        })
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
