use super::*;

#[derive(Document, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestDocument {
    pub id: u128,
    pub other: String,
}

impl Default for TestDocument {
    fn default() -> Self {
        Self {
            id: rand::random(),
            other: rand_utf8(rand::random::<u16>().into()),
        }
    }
}

#[tokio::test]
async fn should_update_document() -> Result<()> {
    #[derive(AnonDB, Clone)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other; unique = true)]
        #[anondb(index = other; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    db.test
        .insert_many((0..50).map(|_| TestDocument::default()))
        .await?;
    let original_doc = TestDocument::default();
    db.test.insert(&original_doc).await?;
    db.test
        .insert_many((0..50).map(|_| TestDocument::default()))
        .await?;
    let start_count = db.test.count().await?;
    db.test
        .update(TestDocument::query(), |doc| {
            if doc.id == original_doc.id {
                let mut doc = doc.clone();
                doc.id = 1;
                Some(doc)
            } else {
                None
            }
        })
        .await?;
    assert_eq!(
        db.test.count().await?,
        start_count,
        "document count changed"
    );

    assert!(
        db.test
            .find_one(TestDocument::query().id().eq(original_doc.id))
            .await?
            .is_none(),
        "original document should be removed"
    );

    let found_doc = db
        .test
        .find_one(TestDocument::query().id().eq(1))
        .await?
        .expect("expected to retrieve a document with updated id");
    assert_eq!(
        found_doc.other, original_doc.other,
        "retrieved document mismatches inserted document"
    );

    Ok(())
}
