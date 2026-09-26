use anyhow::Result;
use futures::TryStreamExt;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Clone, Serialize, Deserialize, PartialEq, Document)]
pub struct TestDocumentV0 {
    pub id0: u128,
    pub id1: u128,
}

impl Default for TestDocumentV0 {
    fn default() -> Self {
        Self {
            id0: rand::random(),
            id1: rand::random(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Document)]
#[document(migrations = migrations1)]
pub struct TestDocumentV1 {
    pub id0: u128,
    pub id1: u128,
    pub id2: u128,
}
fn migrations1<'a>() -> &'static [fn(
    Vec<(String, Vec<u8>)>,
    Vec<String>,
    Vec<u8>,
) -> Result<
    (Vec<(String, Vec<u8>)>, Vec<String>, Vec<u8>),
    MigrationError,
>] {
    &[|key_values, value_fields, bytes| {
        Ok((
            key_values,
            value_fields,
            vec![bytes, postcard::to_stdvec(&999u128).unwrap()].concat(),
        ))
    }]
}

#[tokio::test]
async fn migration_add_field() -> Result<()> {
    let db_path = {
        #[derive(Clone, AnonDB)]
        pub struct DB1<K: KV> {
            #[anondb(primary_key = id0, id1)]
            pub test: Collection<TestDocumentV0, K>,
        }
        let db = DB1::<RedbKV>::new(None).await?;
        for _ in 0..100 {
            db.test.insert(&Default::default()).await?;
        }
        db.test.kv().db_path().to_path_buf()
    };
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0, id1)]
        pub test: Collection<TestDocumentV1, K>,
    }
    let db = DB::<RedbKV>::new(Some(Box::new(db_path))).await?;
    let r = db
        .test
        .find_many(TestDocumentV1::query())
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    for v in r {
        assert_eq!(v.id2, 999);
    }

    Ok(())
}
