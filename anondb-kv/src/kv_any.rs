use std::ops::RangeBounds;

use futures::Stream;
use futures::TryStreamExt;
use futures::future::Either;

use super::*;

/// Run the same expression against whichever backend `$value` holds.
macro_rules! dispatch {
    ($value:expr, $inner:ident => $body:expr) => {
        match $value {
            Self::Memory($inner) => $body,
            Self::Redb($inner) => $body,
        }
    };
}

/// Like `dispatch!`, for expressions evaluating to a stream of backend items. Items are wrapped
/// in `AnyItem` and the two backend stream types are unified with `Either`.
macro_rules! dispatch_stream {
    ($value:expr, $inner:ident => $body:expr) => {
        match $value {
            Self::Memory($inner) => Either::Left($body.map_ok(AnyItem::Memory)),
            Self::Redb($inner) => Either::Right($body.map_ok(AnyItem::Redb)),
        }
    };
}

/// A KV backed by any of the built in backends, chosen at runtime.
///
/// Dispatches with a `match` instead of a trait object, so futures and streams aren't boxed.
#[derive(Debug, Clone)]
pub enum AnyKV {
    Memory(MemoryKV),
    Redb(RedbKV),
}

#[derive(Debug, Clone)]
pub enum AnyKVConfig {
    Memory(<MemoryKV as KV>::Config),
    Redb(<RedbKV as KV>::Config),
}

impl From<<RedbKV as KV>::Config> for AnyKVConfig {
    fn from(value: <RedbKV as KV>::Config) -> Self {
        Self::Redb(value)
    }
}

impl From<<MemoryKV as KV>::Config> for AnyKVConfig {
    fn from(value: <MemoryKV as KV>::Config) -> Self {
        Self::Memory(value)
    }
}

pub enum AnyItem<'tx> {
    Memory(<MemoryKV as KV>::Item<'tx>),
    Redb(<RedbKV as KV>::Item<'tx>),
}

pub enum AnyReadTransaction {
    Memory(<MemoryKV as KV>::ReadTransaction),
    Redb(<RedbKV as KV>::ReadTransaction),
}

pub enum AnyWriteTransaction {
    Memory(<MemoryKV as KV>::WriteTransaction),
    Redb(<RedbKV as KV>::WriteTransaction),
}

impl From<MemoryKV> for AnyKV {
    fn from(kv: MemoryKV) -> Self {
        Self::Memory(kv)
    }
}

impl From<RedbKV> for AnyKV {
    fn from(kv: RedbKV) -> Self {
        Self::Redb(kv)
    }
}

impl OpaqueItem for AnyItem<'_> {
    fn key(&self) -> &[u8] {
        dispatch!(self, item => item.key())
    }

    async fn value(&self) -> &[u8] {
        dispatch!(self, item => item.value().await)
    }
}

impl KV for AnyKV {
    type Item<'tx> = AnyItem<'tx>;
    type ReadTransaction = AnyReadTransaction;
    type WriteTransaction = AnyWriteTransaction;
    type Config = AnyKVConfig;

    /// Without a config the KV is kept in memory.
    async fn new(config_maybe: Option<Self::Config>) -> Result<Self> {
        Ok(match config_maybe {
            None => Self::Memory(MemoryKV::new(None).await?),
            Some(AnyKVConfig::Memory(config)) => Self::Memory(MemoryKV::new(Some(config)).await?),
            Some(AnyKVConfig::Redb(config)) => Self::Redb(RedbKV::new(Some(config)).await?),
        })
    }

    async fn write_tx(&self) -> Result<Self::WriteTransaction> {
        Ok(match self {
            Self::Memory(kv) => AnyWriteTransaction::Memory(kv.write_tx().await?),
            Self::Redb(kv) => AnyWriteTransaction::Redb(kv.write_tx().await?),
        })
    }

    async fn read_tx(&self) -> Result<Self::ReadTransaction> {
        Ok(match self {
            Self::Memory(kv) => AnyReadTransaction::Memory(kv.read_tx().await?),
            Self::Redb(kv) => AnyReadTransaction::Redb(kv.read_tx().await?),
        })
    }

    async fn list_tables(&self) -> Result<Vec<String>> {
        dispatch!(self, kv => kv.list_tables().await)
    }

    async fn list_multimap_tables(&self) -> Result<Vec<String>> {
        dispatch!(self, kv => kv.list_multimap_tables().await)
    }

    fn description(&self) -> String {
        dispatch!(self, kv => kv.description())
    }
}

impl ReadOperations for AnyKV {
    type Item<'tx> = AnyItem<'tx>;

    async fn count(&self, table: &str) -> Result<u64> {
        dispatch!(self, kv => kv.count(table).await)
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<AnyItem<'_>>> {
        Ok(match self {
            Self::Memory(kv) => kv.get(table, key).await?.map(AnyItem::Memory),
            Self::Redb(kv) => kv.get(table, key).await?.map(AnyItem::Redb),
        })
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        dispatch!(self, kv => kv.count_multimap(table).await)
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        dispatch!(self, kv => kv.count_multimap_values(table, key).await)
    }
}

impl WriteOperations for AnyKV {
    type Item<'tx> = AnyItem<'tx>;

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        dispatch!(self, kv => kv.insert(table, key, value).await)
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        dispatch!(self, kv => kv.remove(table, key).await)
    }

    async fn clear(&self, table: &str) -> Result<()> {
        dispatch!(self, kv => kv.clear(table).await)
    }

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        dispatch!(self, kv => kv.rename_table(table, new_name).await)
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        dispatch!(self, kv => kv.insert_multimap(table, key, value).await)
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        dispatch!(self, kv => kv.remove_multimap_value(table, key, value).await)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        dispatch!(self, kv => kv.remove_multimap(table, key).await)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        dispatch!(self, kv => kv.clear_multimap(table).await)
    }
}

impl RangeReadOperations for AnyKV {
    type Item<'tx> = AnyItem<'tx>;

    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + Send + Sync,
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, kv => kv.range(table, range).await?))
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + Send + Sync,
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, kv => kv.range_multimap(table, range).await?))
    }

    async fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, kv => kv.get_multimap(table, key).await?))
    }
}

impl ReadOperations for AnyReadTransaction {
    type Item<'tx> = AnyItem<'tx>;

    async fn count(&self, table: &str) -> Result<u64> {
        dispatch!(self, tx => tx.count(table).await)
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<AnyItem<'_>>> {
        Ok(match self {
            Self::Memory(tx) => tx.get(table, key).await?.map(AnyItem::Memory),
            Self::Redb(tx) => tx.get(table, key).await?.map(AnyItem::Redb),
        })
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        dispatch!(self, tx => tx.count_multimap(table).await)
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        dispatch!(self, tx => tx.count_multimap_values(table, key).await)
    }
}

impl RangeReadOperations for AnyReadTransaction {
    type Item<'tx> = AnyItem<'tx>;

    async fn range(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + Send + Sync,
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, tx => tx.range(table, range).await?))
    }

    async fn range_multimap(
        &self,
        table: &str,
        range: impl RangeBounds<Vec<u8>> + Send + Sync,
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, tx => tx.range_multimap(table, range).await?))
    }

    async fn get_multimap(
        &self,
        table: &str,
        key: &[u8],
    ) -> Result<impl Stream<Item = Result<AnyItem<'_>>>> {
        Ok(dispatch_stream!(self, tx => tx.get_multimap(table, key).await?))
    }
}

impl ReadOperations for AnyWriteTransaction {
    type Item<'tx> = AnyItem<'tx>;

    async fn count(&self, table: &str) -> Result<u64> {
        dispatch!(self, tx => tx.count(table).await)
    }

    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<AnyItem<'_>>> {
        Ok(match self {
            Self::Memory(tx) => tx.get(table, key).await?.map(AnyItem::Memory),
            Self::Redb(tx) => tx.get(table, key).await?.map(AnyItem::Redb),
        })
    }

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        dispatch!(self, tx => tx.count_multimap(table).await)
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        dispatch!(self, tx => tx.count_multimap_values(table, key).await)
    }
}

impl WriteOperations for AnyWriteTransaction {
    type Item<'tx> = AnyItem<'tx>;

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        dispatch!(self, tx => tx.insert(table, key, value).await)
    }

    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        dispatch!(self, tx => tx.remove(table, key).await)
    }

    async fn clear(&self, table: &str) -> Result<()> {
        dispatch!(self, tx => tx.clear(table).await)
    }

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        dispatch!(self, tx => tx.rename_table(table, new_name).await)
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        dispatch!(self, tx => tx.insert_multimap(table, key, value).await)
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        dispatch!(self, tx => tx.remove_multimap_value(table, key, value).await)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        dispatch!(self, tx => tx.remove_multimap(table, key).await)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        dispatch!(self, tx => tx.clear_multimap(table).await)
    }
}

impl WriteTx for AnyWriteTransaction {
    type Item<'tx> = AnyItem<'tx>;

    async fn commit(self) -> Result<()> {
        dispatch!(self, tx => tx.commit().await)
    }
}
