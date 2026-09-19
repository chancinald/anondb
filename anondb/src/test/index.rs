use futures::TryStreamExt;

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Document)]
pub struct TestDocument2 {
    pub id: u128,
    pub other: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Document)]
pub struct OtherDocument {
    pub id: u128,
    pub other: u64,
}

#[derive(AnonDB)]
pub struct DB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = id, other; unique = true)]
    pub _test_collection: Collection<TestDocument2, K>,
    #[anondb(primary_key = id)]
    #[anondb(index = other)]
    pub other_collection: Collection<OtherDocument, K>,
}

#[tokio::test]
async fn should_use_non_unique_index() -> Result<()> {
    let db = DB::<MemoryKV>::new(None).await?;
    let doc = OtherDocument { id: 99, other: 99 };
    let doc2 = OtherDocument {
        id: 200,
        other: 200,
    };
    db.other_collection.insert(&doc).await?;
    db.other_collection.insert(&doc2).await?;

    let out = db
        .other_collection
        .find_many(OtherDocument::query().other(0..))
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    assert_eq!(out.len(), 2);
    assert_eq!(out.get(0).unwrap().other, 99);
    assert_eq!(out.get(1).unwrap().other, 200);

    Ok(())
}

#[tokio::test]
async fn should_automatically_rebuild_indices() -> Result<()> {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Document)]
    pub struct TestDocument {
        pub id: u128,
        pub other: String,
    }
    impl Default for TestDocument {
        fn default() -> Self {
            Self {
                id: rand::random(),
                other: rand::random::<u128>().to_string(),
            }
        }
    }
    let (db_path, orig_table_count) = {
        #[derive(AnonDB)]
        pub struct DB<K: KV> {
            #[anondb(primary_key = id)]
            #[anondb(index = id, other; unique = true)]
            pub test_collection: Collection<TestDocument, K>,
        }
        let db = DB::<RedbKV>::new(None).await?;
        for _ in 0..100 {
            db.test_collection.insert(&Default::default()).await?;
        }
        let orig_table_count = db.test_collection.kv().list_tables().await?.len();
        assert_eq!(
            db.test_collection.kv().list_multimap_tables().await?.len(),
            0
        );
        (
            db.test_collection.kv().db_path().to_path_buf(),
            orig_table_count,
        )
    };
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other)]
        pub test_collection: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::new(Some(Box::new(db_path))).await?;
    assert_eq!(
        db.test_collection.kv().list_tables().await?.len(),
        orig_table_count - 1
    );
    assert_eq!(
        db.test_collection.kv().list_multimap_tables().await?.len(),
        1
    );

    Ok(())
}
