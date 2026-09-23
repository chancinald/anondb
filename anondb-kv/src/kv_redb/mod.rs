mod maybe;
mod read_tx;
mod table;
mod write_tx;

use maybe::*;
use read_tx::*;
use table::*;
use write_tx::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use futures::Stream;
use futures::TryStreamExt;
use futures::future::Either;
use tokio::sync::RwLock;

use redb::*;

use super::*;

#[derive(Debug, Clone)]
pub struct RedbKV {
    db: Arc<Database>,
    path: PathBuf,
}

impl RedbKV {
    pub fn db_path(&self) -> &std::path::Path {
        &self.path
    }
}

impl KV for RedbKV {
    type ReadTransaction = RedbReadTransaction;
    type WriteTransaction = RedbWriteTransaction;
    type Item<'tx> = RedbItem<'tx>;
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
            db: Arc::new(
                if std::fs::exists(path.as_path())? {
                    Database::open(path.as_path())
                } else {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    Database::create(path.as_path())
                }
                .with_context(|| "initializing anondb Redb kv backend")?,
            ),
            path: path.to_path_buf(),
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        Ok(RedbWriteTransaction {
            write: self.db.begin_write()?,
        })
    }

    async fn read_tx(&self) -> Result<Self::ReadTransaction> {
        Ok(RedbReadTransaction {
            read: self.db.begin_read()?,
            tables: RwLock::new(HashMap::default()),
            tables_multimap: RwLock::new(HashMap::default()),
        })
    }

    async fn list_tables(&self) -> Result<Vec<String>> {
        Ok(self
            .db
            .begin_read()?
            .list_tables()?
            .map(|v| v.name().to_string())
            .collect())
    }

    async fn list_multimap_tables(&self) -> Result<Vec<String>> {
        Ok(self
            .db
            .begin_read()?
            .list_multimap_tables()?
            .map(|v| v.name().to_string())
            .collect())
    }

    fn description(&self) -> String {
        format!("Redb backend\n{}", self.path.to_string_lossy())
    }
}

/// Operations occuring outside of a transaction. "One and done" operations.
impl ReadOperations for RedbKV {
    type Item<'tx> = RedbItem<'tx>;
    async fn count(&self, table: &str) -> Result<u64> {
        let tx: <Self as KV>::ReadTransaction = self.read_tx().await?;
        tx.count(table).await
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<Self::Item<'_>>> {
        let read = self.db.begin_read()?;
        let Some(table) = read_table(&read, table)? else {
            return Ok(None);
        };
        Ok(table.get(key)?.map(|v| RedbItem::from((key, v))))
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

impl WriteOperations for RedbKV {
    type Item<'tx> = RedbItem<'tx>;
    async fn clear(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        tx.clear(table).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.insert(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.remove(table, key).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.rename_table(table, new_name).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.insert_multimap(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.remove_multimap_value(table, key, value).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        let out = tx.remove_multimap(table, key).await?;
        tx.commit().await?;
        Ok(out)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        let tx: <Self as KV>::WriteTransaction = self.write_tx().await?;
        tx.clear_multimap(table).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl RangeReadOperations for RedbKV {
    type Item<'tx>
        = RedbItem<'tx>
    where
        Self: 'tx;
    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item<'_>>>> {
        let read = self.db.begin_read()?;
        let Some(table) = read_table(&read, table)? else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        Ok(Either::Right(futures::stream::iter(
            table
                .range::<&[u8]>(to_redb_bounds(range.start_bound(), range.end_bound()))?
                .map(|v| Ok(RedbItem::from(v?))),
        )))
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item<'_>>>> {
        let read = self.db.begin_read()?;
        let Some(table) = read_multimap_table(&read, table)? else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        Ok(Either::Right(
            futures::stream::iter(
                table
                    .range::<&[u8]>(to_redb_bounds(range.start_bound(), range.end_bound()))?
                    .map(|v| {
                        let (key, value) = v?;
                        let key = key.value().to_vec();
                        Ok::<_, anyhow::Error>(futures::stream::iter(value.into_iter().map(
                            move |v| {
                                let item = v?;
                                Ok(RedbItem::from((key.as_slice(), item)))
                            },
                        )))
                    }),
            )
            .try_flatten(),
        ))
    }

    async fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<Self::Item<'_>>>> {
        let read = self.db.begin_read()?;
        let Some(table) = read_multimap_table(&read, table)? else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        Ok(Either::Right(async_stream::try_stream! {
            for doc in table.get(key)? {
                let doc = doc?;
                let item = Self::Item::from((key, doc));
                yield item;
            }
        }))
    }
}
