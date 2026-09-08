use rpds::RedBlackTreeMapSync;

use super::*;

pub struct MemoryKVReadTransaction {
    pub(crate) tables: HashMap<String, RedBlackTreeMapSync<Vec<u8>, Vec<u8>>>,
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
}

impl RangeReadOperation for MemoryKVReadTransaction {
    type Item = MemoryKVItem;
    async fn range<'a>(
        &'a self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + 'a,
    ) -> Result<impl Iterator<Item = Result<Self::Item>> + 'a> {
        let Some(table) = self.tables.get(table) else {
            anyhow::bail!("table not found \"{table}\"")
        };
        Ok(table.range(range).map(|(key, value)| {
            Ok(MemoryKVItem {
                key: key.clone(),
                value: value.clone(),
            })
        }))
    }
}
