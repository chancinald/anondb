mod maybe;
mod read_tx;
mod write_tx;

use maybe::*;
use read_tx::*;
use write_tx::*;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use redb::*;

use super::*;

fn tabledef(name: &str) -> TableDefinition<'_, &'static [u8], &'static [u8]> {
    TableDefinition::new(name)
}

// fn tabledef_multimap(name: &str) -> MultimapTableDefinition<'_, &'static [u8], &'static [u8]> {
//     MultimapTableDefinition::new(name)
// }

#[derive(Debug, Clone)]
pub struct RedbKV<'a> {
    db: Arc<Database>,
    _phantom: PhantomData<&'a ()>,
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
}
