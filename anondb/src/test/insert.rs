use super::*;

type Custom = ();
#[derive(Document, Debug, Clone, PartialEq)]
pub struct TestDocument {
    pub id: u128,
    pub other: String,
    #[anondb(skip, default)]
    pub v: Option<Custom>,
}

impl Default for TestDocument {
    fn default() -> Self {
        Self {
            id: rand::random(),
            other: rand_utf8(rand::random::<u16>().into()),
            v: None,
        }
    }
}

#[tokio::test]
async fn should_insert_document() -> Result<()> {
    #[derive(AnonDB, Clone)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    let doc = TestDocument::default();

    assert_eq!(db.test.count().await?, 0);
    db.test.insert(&doc).await?;
    assert_eq!(db.test.count().await?, 1);

    let found_doc = db
        .test
        .find_one(TestDocument::query())
        .await?
        .expect("expected to retrieve a document using empty query");
    assert_eq!(
        found_doc, doc,
        "retrieved document mismatches inserted document"
    );

    Ok(())
}
