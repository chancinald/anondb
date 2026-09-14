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

    async fn rename_table(&self, table: &str, new_name: &str) -> Result<()> {
        self.write.delete_table(tabledef(new_name))?;
        self.write
            .rename_table(tabledef(table), tabledef(new_name))?;
        Ok(())
    }

    async fn insert_multimap(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        table.insert(key, value)?;
        Ok(())
    }

    async fn remove_multimap_value(&self, table: &str, key: &[u8], value: &[u8]) -> Result<bool> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        Ok(table.remove(key, value)?)
    }

    async fn remove_multimap(&self, table: &str, key: &[u8]) -> Result<bool> {
        let mut table = self.write.open_multimap_table(tabledef_multimap(table))?;
        let removed = table.remove_all(key)?;
        Ok(removed.len() > 0)
    }

    async fn clear_multimap(&self, table: &str) -> Result<()> {
        self.write.delete_multimap_table(tabledef_multimap(table))?;
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

    async fn count_multimap(&self, table: &str) -> Result<u64> {
        let table = self.write.open_multimap_table(tabledef_multimap(&table))?;
        Ok(table.len()? as u64)
    }

    async fn count_multimap_values(&self, table: &str, key: &[u8]) -> Result<u64> {
        let table = self.write.open_multimap_table(tabledef_multimap(&table))?;
        Ok(table.get(key)?.len() as u64)
    }
}
