use super::*;

pub struct RedbWriteTransaction<'a> {
    pub write: redb::WriteTransaction,
    pub _phantom: PhantomData<&'a ()>,
}

impl<'a> WriteTx for RedbWriteTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn commit(self) -> Result<()> {
        self.write.commit()?;
        Ok(())
    }
}

impl<'a> WriteOperations for RedbWriteTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn remove(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut table = self.write.open_table(tabledef(table))?;
        Ok(table.remove(key)?.map(|v| v.value().to_vec()))
    }

    async fn insert(&self, table: &str, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut table = self.write.open_table(tabledef(table))?;
        Ok(table.insert(key, value)?.map(|v| v.value().to_vec()))
    }

    async fn clear(&self, table: &str) -> Result<()> {
        self.write.delete_table(tabledef(table))?;
        Ok(())
    }
}

impl<'a> ReadOperations for RedbWriteTransaction<'a> {
    type Item = RedbItem<'a>;
    async fn get(&self, table: &str, key: &[u8]) -> Result<Option<Self::Item>> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table
            .get(key)?
            .map(|v| (key.to_vec(), v.value().to_vec()).into()))
    }

    async fn count(&self, table: &str) -> Result<u64> {
        let table = self.write.open_table(tabledef(table))?;
        Ok(table.len()?)
    }
}
