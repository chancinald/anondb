mod maybe;
mod read_tx;
mod write_tx;

use maybe::*;
use read_tx::*;
use write_tx::*;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::RwLock;

use redb::*;

use super::*;

fn tabledef(name: &str) -> TableDefinition<'_, &'static [u8], &'static [u8]> {
    TableDefinition::new(name)
}

fn tabledef_multimap(name: &str) -> MultimapTableDefinition<'_, &'static [u8], &'static [u8]> {
    MultimapTableDefinition::new(name)
}

#[derive(Debug, Clone)]
pub struct RedbKV<'a> {
    db: Arc<Database>,
    path: PathBuf,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> RedbKV<'a> {
    pub fn db_path(&self) -> &std::path::Path {
        &self.path
    }
}

impl<'a> KV for RedbKV<'a> {
    type ReadTransaction = RedbReadTransaction<'a>;
    type WriteTransaction = RedbWriteTransaction<'a>;
    type Item = RedbItem<'a>;
    type Config = Box<std::path::PathBuf>;

    async fn new(config_maybe: Option<Self::Config>) -> Result<Self> {
        let path = config_maybe.unwrap_or_else(|| {
            #[cfg(debug_assertions)]
            {
                Box::new(
                    std::env::temp_dir()
                        .join(format!("db-{}.redb", rand::random::<u128>().to_string())),
                )
            }
            #[cfg(not(debug_assertions))]
            panic!("refusing to start without redb database path")
        });
        Ok(Self {
            db: Arc::new(if std::fs::exists(path.as_path())? {
                Database::open(path.as_path())?
            } else {
                Database::create(path.as_path())?
            }),
            path: path.to_path_buf(),
            _phantom: PhantomData::default(),
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        Ok(RedbWriteTransaction {
            write: self.db.begin_write()?,
            _phantom: PhantomData::default(),
        })
    }

    async fn read_tx(&self) -> Result<Self::ReadTransaction> {
        Ok(RedbReadTransaction {
            read: self.db.begin_read()?,
            tables: RwLock::new(HashMap::default()),
            tables_multimap: RwLock::new(HashMap::default()),
            _phantom: PhantomData::default(),
        })
    }
}

/// Operations occuring outside of a transaction. "One and done" operations.
impl<'a> ReadOperations for RedbKV<'a> {
    type Item = RedbItem<'a>;
    async fn count(&self, table: &str) -> Result<u64> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx().await?;
        tx.count(table).await
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<Self::Item>> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx().await?;
        tx.get(table, key).await
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        self.read_tx().await?.count_multimap(table).await
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        self.read_tx()
            .await?
            .count_multimap_values(table, key)
            .await
    }
}

impl<'a> WriteOperations for RedbKV<'a> {
    type Item = RedbItem<'a>;
    async fn clear(&self, table: &str) -> Result<()> {
        let tx = self.write_tx().await?;
        tx.clear(table).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx = self.write_tx().await?;
        let out = tx.insert(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx = self.write_tx().await?;
        let out = tx.remove(table, key).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        let tx = self.write_tx().await?;
        let out = tx.rename_table(table, new_name).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tx = self.write_tx().await?;
        let out = tx.insert_multimap(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let tx = self.write_tx().await?;
        let out = tx.remove_multimap_value(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        let tx = self.write_tx().await?;
        let out = tx.remove_multimap(table, key).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        let tx = self.write_tx().await?;
        tx.clear_multimap(table).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl<'a> RangeReadOperations for RedbKV<'a> {
    type Item = RedbItem<'a>;
    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(async_stream::try_stream! {
            let tx = self.read_tx().await?;
            let out = tx.range(table, range).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(doc) = stream.next().await.transpose()? {
                yield doc;
            }
        })
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(async_stream::try_stream! {
            let tx = self.read_tx().await?;
            let out = tx.range_multimap(table, range).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(doc) = stream.next().await.transpose()? {
                yield doc;
            }
        })
    }

    async fn get_multimap<'b>(
        &'b self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<Self::Item>> + 'b> {
        let table = table.to_string();
        let key = key.to_vec();
        Ok(async_stream::try_stream! {
            let tx: <Self as KV>::ReadTransaction = self.read_tx().await?;
            let out = tx.get_multimap(&table, key.as_slice()).await?;
            let mut stream = std::pin::pin!(out);
            while let Some(item) = stream.next().await.transpose()? {
                yield item;
            }
        })
    }
}
