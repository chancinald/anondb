use redb::*;
use serde::Serialize;

use super::TransactionOperations;

pub struct JournaledTable<'a, K>
where
    K: Key + Serialize + 'static,
    for<'b> K::SelfType<'b>: ToOwned<Owned = K>,
{
    table: Table<'a, K, K>,
    journal_channel: flume::Sender<TransactionOperations<K>>,
}

impl<'a, K> JournaledTable<'a, K>
where
    K: Key + Serialize + 'static,
    for<'b> K::SelfType<'b>: ToOwned<Owned = K>,
{
    pub fn new(
        table: Table<'a, K, K>,
        journal_channel: flume::Sender<TransactionOperations<K>>,
    ) -> Self {
        Self {
            table,
            journal_channel,
        }
    }

    pub fn insert<'k, 'v>(
        &mut self,
        key: K::SelfType<'a>,
        value: K::SelfType<'a>,
    ) -> Result<Option<AccessGuard<K>>> {
        let name = self.table.name().to_string();
        let out = self.table.insert(&key, &value)?;
        // TODO: return a result here
        // actually we want to match on the error above and not even try to journal
        // if the insert succeeds and the journal errors we... crash? error?
        self.journal_channel
            .send(TransactionOperations::Insert(
                name,
                key.to_owned(),
                value.to_owned(),
            ))
            .unwrap();
        Ok(out)
    }
}
