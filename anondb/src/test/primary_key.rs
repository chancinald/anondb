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

#[tokio::test]
async fn should_fail_to_insert_duplicate_primary_key() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other; unique = true)]
        pub test_collection: Collection<TestDocument2, K>,
        #[anondb(primary_key = id)]
        #[anondb(index = other)]
        pub _other_collection: Collection<OtherDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;

    let doc = TestDocument2 {
        id: 99,
        other: "".into(),
    };
    db.test_collection.insert(&doc).await?;
    db.test_collection
        .insert(&doc)
        .await
        .expect_err("Should fail to insert duplicate primary key");

    Ok(())
}

#[tokio::test]
async fn compound_primary_key() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0, id1, id2)]
        pub test: Collection<BigTestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;

    let mut doc0 = BigTestDocument::default();

    db.test.insert(&doc0).await?;

    doc0.id1 = rand::random();

    db.test.insert(&doc0).await?;
    db.test
        .insert(&doc0)
        .await
        .expect_err("Should fail to insert duplicate compound primary key");

    let doc = db
        .test
        .find_one(BigTestDocument::query().id1().eq(doc0.id1))
        .await?
        .expect("unable to find inserted doc");

    assert_eq!(doc, doc0);
    Ok(())
}

// #[test]
// fn fail_to_start_multiple_primary_keys() -> Result<()> {
//     #[derive(Debug, AnonDB)]
//     pub struct DB<K: KV> {
//         #[anondb(primary_key = id0, id1, id2)]
//         #[anondb(index = id0; primary = true)]
//         pub test: Collection<TestDocument, K>,
//     }
//     DB::<RedbKV>::in_memory(None)?; //.expect_err("Should failed to start with multiple primary keys");
//     Ok(())
// }
