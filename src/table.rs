use anyhow::Result;
use redb::*;

use super::TransactionOperations;

pub struct JournaledTable<'a, K, V>
where
    K: Key + 'static,
    V: Value + 'static,
{
    table: Table<'a, K, V>,
    journal_channel: flume::Sender<TransactionOperations>,
}

impl<'a, K, V> JournaledTable<'a, K, V>
where
    K: Key + 'static,
    V: Value + 'static,
{
    pub fn new(
        table: Table<'a, K, V>,
        journal_channel: flume::Sender<TransactionOperations>,
    ) -> Self {
        Self {
            table,
            journal_channel,
        }
    }

    pub fn insert<'k, 'v>(
        &mut self,
        key: K::SelfType<'a>,
        value: V::SelfType<'a>,
    ) -> Result<Option<AccessGuard<V>>> {
        let name = self.table.name().to_string();
        let out = self.table.insert(&key, &value)?;
        // TODO: return a result here
        // actually we want to match on the error above and not even try to journal
        // if the insert succeeds and the journal errors we... crash? error?
        self.journal_channel.send(TransactionOperations::Insert(
            name,
            K::as_bytes(&key).as_ref().to_vec(),
            V::as_bytes(&value).as_ref().to_vec(),
            // key.to_owned(),
            // value.to_owned(),
        ))?;
        Ok(out)
    }

    pub fn remove<'k>(&mut self, key: K::SelfType<'k>) -> Result<Option<AccessGuard<V>>> {
        let name = self.table.name().to_string();
        let out = self.table.remove(&key)?;
        self.journal_channel.send(TransactionOperations::Remove(
            name,
            K::as_bytes(&key).as_ref().to_vec(),
        ))?;
        Ok(out)
    }
}
