use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;

use either::Either;

use super::*;

pub struct RedbReadTransaction<'a> {
    pub read: redb::ReadTransaction,
    pub tables: RwLock<HashMap<String, Arc<ReadOnlyTable<&'static [u8], &'static [u8]>>>>,
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

impl<'a> RangeReadOperation for RedbReadTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn range<'b>(
        &'b self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + 'b,
    ) -> Result<impl Iterator<Item = Result<Self::Item>> + 'b> {
        match self.read_table(table).await? {
            Some(table) => Ok(Either::Left(
                table
                    .range::<&[u8]>(to_redb_bounds(range.start_bound(), range.end_bound()))?
                    .map(|v| Ok(RedbItem::from(v?))),
            )),
            None => Ok(Either::Right(std::iter::empty())),
        }
    }
}
