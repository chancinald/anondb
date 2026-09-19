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
    pub test_collection: Collection<TestDocument2, K>,
    #[anondb(primary_key = id)]
    #[anondb(index = other)]
    pub _other_collection: Collection<OtherDocument, K>,
}

#[tokio::test]
async fn should_query_collection() -> Result<()> {
    let db = DB::<MemoryKV>::new(None).await?;

    let doc = TestDocument2 {
        id: 99,
        other: "".into(),
    };
    db.test_collection.insert(&doc).await?;

    {
        let out = db
            .test_collection
            .find_one(TestDocument2::query().id(80..).other(""))
            .await?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }
    {
        let out = db
            .test_collection
            .find_one(TestDocument2::query().id(1000..))
            .await?;
        assert!(out.is_none());
    }
    {
        let out = db
            .test_collection
            .find_one(TestDocument2::query().id(99))
            .await?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }

    Ok(())
}
