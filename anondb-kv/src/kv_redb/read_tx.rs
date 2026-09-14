use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;

use futures::Stream;
use futures::TryStreamExt;
use futures::future::Either;

use super::*;

pub struct RedbReadTransaction<'a> {
    pub read: redb::ReadTransaction,
    pub tables: RwLock<HashMap<String, Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>>,
    pub tables_multimap:
        RwLock<HashMap<String, Arc<ReadOnlyMultimapTable<&'static [u8], &'static [u8]>>>>,
    pub _phantom: PhantomData<&'a ()>,
}

impl<'a> RedbReadTransaction<'a> {
    pub async fn read_table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>> {
        if let Some(table) = self.tables.read().await.get(name) {
            return Ok(Some(table.clone()));
        }
        let table = match self.read.open_table(tabledef(name)) {
            Ok(t) => t,
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    return Ok(None);
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
        let table = Arc::new(table);
        self.tables.write().await.insert(name.into(), table.clone());
        Ok(Some(table))
    }

    pub async fn read_multimap_table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<ReadOnlyMultimapTable<&'static [u8], &'static [u8]>>>> {
        if let Some(table) = self.tables_multimap.read().await.get(name) {
            return Ok(Some(table.clone()));
        }
        let table = match self.read.open_multimap_table(tabledef_multimap(name)) {
            Ok(t) => t,
            Err(e) => {
                if matches!(e, TableError::TableDoesNotExist(_)) {
                    return Ok(None);
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
        let table = Arc::new(table);
        self.tables_multimap
            .write()
            .await
            .insert(name.into(), table.clone());
        Ok(Some(table))
    }
}

impl<'a> ReadOperations for RedbReadTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<Self::Item>> {
        match self.read_table(table).await? {
            Some(table) => Ok(table.get(key)?.map(|v| (key, v).into())),
            None => Ok(None),
        }
    }

    async fn count(&self, table: &str) -> Result<u64> {
        match self.read_table(table).await? {
            Some(table) => Ok(table.len()?),
            None => Ok(0),
        }
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        match self.read_multimap_table(table).await? {
            Some(table) => Ok(table.len()?),
            None => Ok(0),
        }
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        match self.read_multimap_table(table).await? {
            Some(table) => Ok(table.get(key)?.len()),
            None => Ok(0),
        }
    }
}

use std::ops::Bound;

fn to_redb_bounds<'a>(
    start: Bound<&'a Vec<u8>>,
    end: Bound<&'a Vec<u8>>,
) -> (Bound<&'a [u8]>, Bound<&'a [u8]>) {
    let map = |b: Bound<&'a Vec<u8>>| match b {
        Bound::Included(v) => Bound::Included(v.as_slice()),
        Bound::Excluded(v) => Bound::Excluded(v.as_slice()),
        Bound::Unbounded => Bound::Unbounded,
    };
    (map(start), map(end))
}

impl<'a> RangeReadOperations for RedbReadTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        match self.read_table(table).await? {
            Some(table) => Ok(Either::Left(futures::stream::iter(
                table
                    .range::<&[u8]>(to_redb_bounds(range.start_bound(), range.end_bound()))?
                    .map(|v| Ok(RedbItem::from(v?))),
            ))),
            None => Ok(Either::Right(futures::stream::empty())),
        }
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        match self.read_multimap_table(table).await? {
            Some(table) => Ok(Either::Left(
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
            )),
            None => Ok(Either::Right(futures::stream::empty())),
        }
    }

    async fn get_multimap<'b>(
        &'b self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<Self::Item>> + 'b> {
        let Some(table) = self.read_multimap_table(table).await? else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        let key = key.to_vec();
        Ok(Either::Right(async_stream::try_stream! {
            for doc in table.get(key.as_slice())? {
                let doc = doc?;
                let item = Self::Item::from((key.as_slice(), doc));
                yield item;
            }
        }))
    }
}
