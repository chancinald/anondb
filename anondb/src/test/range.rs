use super::*;

#[test]
fn should_insert_document() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0)]
        #[anondb(index = id1, id2, id3, id4; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc = TestDocument::default();

    for _ in 0..100 {
        db.test.insert(&TestDocument::default())?;
    }

    let doc = db
        .test
        .find_many(TestDocument::query().id3(99..u128::MAX))?
        .collect::<Vec<_>>();
    println!("found {}", doc.len());

    Ok(())
}
