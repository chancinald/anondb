use futures::future::Either;
use rpds::RedBlackTreeMapSync;

use super::*;

pub struct MemoryKVReadTransaction {
    pub(crate) tables: HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
    pub(crate) tables_multi: HashMap<String, RedBlackTreeMapSync<Vec<u8>, HashSet<Vec<u8>>>>,
}

impl ReadOperations for MemoryKVReadTransaction {
    type Item = MemoryKVItem;
    async fn count(&self, table: &str) -> Result<u64> {
        match self.tables.get(table) {
            Some(table) => Ok(table.size() as u64),
            None => {
                log::warn!("table \"{table}\" not found");
                Ok(0)
            }
        }
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<MemoryKVItem>> {
        if let Some(table) = self.tables.get(table)
            && let Some(item) = table.get(key)
        {
            Ok(Some(MemoryKVItem {
                key: key.to_vec(),
                value: item.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        Ok(self
            .tables_multi
            .get(table)
            .and_then(|v| Some(v.size() as u64))
            .unwrap_or(0u64))
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        Ok(self
            .tables_multi
            .get(table)
            .and_then(|v| v.get(key))
            .and_then(|v| Some(v.len() as u64))
            .unwrap_or(0u64))
    }

    async fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        Ok(
            if let Some(table) = self.tables_multi.get(table)
                && let Some(entry) = table.get(key)
            {
                Either::Left(async_stream::try_stream! {
                    for doc in entry.iter() {
                        let item = MemoryKVItem {
                            key: key.to_vec(),
                            value: doc.clone(),
                        };
                        yield item;
                    }
                })
            } else {
                Either::Right(futures::stream::empty())
            },
        )
    }
}

impl RangeReadOperations for MemoryKVReadTransaction {
    type Item = MemoryKVItem;
    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        let Some(table) = self.tables.get(table) else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        Ok(Either::Right(futures::stream::iter(
            table.range(range).map(|(key, value)| {
                Ok(MemoryKVItem {
                    key: key.clone(),
                    value: value.clone(),
                })
            }),
        )))
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>>,
    ) -> Result<impl Stream<Item = Result<Self::Item>>> {
        let Some(table) = self.tables_multi.get(table) else {
            return Ok(Either::Left(futures::stream::empty()));
        };
        Ok(Either::Right(futures::stream::iter(
            table.range(range).flat_map(|(key, set)| {
                set.iter().map(|value| {
                    Ok(MemoryKVItem {
                        key: key.clone(),
                        value: value.clone(),
                    })
                })
            }),
        )))
    }
}
