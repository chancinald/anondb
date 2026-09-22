use super::*;

#[tokio::test]
async fn should_use_default_value() -> Result<()> {
    #[derive(Document, Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct TestDocument {
        pub id: u128,
        #[anondb(skip, default)]
        pub other: String,
    }

    #[derive(AnonDB, Clone)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    db.test
        .insert(TestDocument {
            id: 1,
            other: "fakljdlfa".to_string(),
        })
        .await?;
    let doc = db
        .test
        .find_one(TestDocument::query())
        .await?
        .expect("document should exist");
    assert_eq!(doc.other, String::default());

    Ok(())
}

#[tokio::test]
async fn should_use_custom_default_value() -> Result<()> {
    fn custom_default() -> String {
        "hello".to_string()
    }
    #[derive(Document, Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct TestDocument {
        pub id: u128,
        #[anondb(skip, default = "custom_default")]
        pub other: String,
    }

    #[derive(AnonDB, Clone)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    db.test
        .insert(TestDocument {
            id: 1,
            other: "fakljdlfa".to_string(),
        })
        .await?;
    let doc = db
        .test
        .find_one(TestDocument::query())
        .await?
        .expect("document should exist");
    assert_eq!(doc.other, custom_default());
    Ok(())
}
