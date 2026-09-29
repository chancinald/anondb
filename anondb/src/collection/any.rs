use std::collections::BTreeMap;

use super::*;

pub trait AnyCollection<K: KV>: Send + Sync {
    fn name(&self) -> &str;
    fn indices(&self) -> Vec<Box<dyn AnyIndex>>;
    fn metatable(&self) -> String {
        format!("{NAME_SEPARATOR}metadata_{}", self.name())
    }
    fn description(
        &self,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<CollectionDescription>> + Send + Sync>> {
        let kv = self.kv();
        let metatable = self.metatable();
        Box::pin(async move {
            if let Some(item) = kv.get(&metatable, DESCRIPTION_KEY).await? {
                Ok(postcard::from_bytes(&item.value().await)?)
            } else {
                anyhow::bail!("description document is not set")
            }
        })
    }
    fn primary_key_field_names(&self) -> &Vec<(String, OwnedLexStats)>;
    fn kv(&self) -> K;
    fn parse_key_value(
        &self,
        key: &[u8],
        value: &[u8],
    ) -> Result<(
        Vec<(String, String, Box<dyn ValidDBType>)>,
        BTreeMap<String, (String, Box<dyn ValidDBType>)>,
    )>;
    fn parse_key(&self, key: &[u8]) -> Result<Vec<(String, String, Box<dyn ValidDBType>)>>;
}

impl<T: CollectionDocument + 'static, K: KV> AnyCollection<K> for Collection<T, K> {
    fn name(&self) -> &str {
        &self.name
    }

    fn indices(&self) -> Vec<Box<dyn AnyIndex>> {
        self.indices
            .iter()
            .cloned()
            .map(|v| Box::new(v) as Box<dyn AnyIndex>)
            .collect::<Vec<_>>()
    }

    fn primary_key_field_names(&self) -> &Vec<(String, OwnedLexStats)> {
        self.primary_index.fields()
    }

    fn kv(&self) -> K {
        self.kv.clone()
    }

    fn parse_key_value(
        &self,
        key: &[u8],
        value: &[u8],
    ) -> Result<(
        Vec<(String, String, Box<dyn ValidDBType>)>,
        BTreeMap<String, (String, Box<dyn ValidDBType>)>,
    )> {
        let mut entries = self.primary_index.deserialize(key, value)?.into_map();
        let mut key_entries = Vec::with_capacity(self.primary_key_field_names().len());
        for (name, _) in self.primary_key_field_names().iter() {
            let entry = entries
                .remove(name)
                .expect("should contain primary key entry");
            key_entries.push((name.to_string(), entry.0, entry.1));
        }
        Ok((key_entries, entries))
    }

    fn parse_key(&self, key: &[u8]) -> Result<Vec<(String, String, Box<dyn ValidDBType>)>> {
        Ok(self.primary_index.deserialize_key(key)?)
    }

    // fn parse_key(&self, key: &[u8]) -> Result<Vec<(String, String, Box<dyn ValidDBType>)>> {
    //     let mut entries = self.primary_index.deserialize(key, value).into_map();
    //     let mut key_entries = Vec::with_capacity(self.primary_key_field_names().len());
    //     for (name, _) in self.primary_key_field_names().iter() {
    //         let entry = entries
    //             .remove(name)
    //             .expect("should contain primary key entry");
    //         key_entries.push((name.to_string(), entry.0, entry.1));
    //     }
    //     Ok(key_entries)
    // }
}
