use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Document)]
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

#[test]
fn should_insert_document() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc = TestDocument::default();

    assert_eq!(db.test.count()?, 0);
    db.test.insert(&doc)?;
    assert_eq!(db.test.count()?, 1);

    let found_doc = db
        .test
        .find_one(TestDocument::query())?
        .expect("expected to retrieve a document using empty query");
    assert_eq!(
        found_doc, doc,
        "retrieved document mismatches inserted document"
    );

    Ok(())
}
