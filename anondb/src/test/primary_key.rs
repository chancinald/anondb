use super::*;

#[test]
fn compound_primary_key() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0, id1, id2)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;

    let mut doc0 = TestDocument::default();

    db.test.insert(&doc0)?;

    doc0.id1 = rand::random();

    db.test.insert(&doc0)?;
    db.test
        .insert(&doc0)
        .expect_err("Should fail to insert duplicate compound primary key");

    let doc = db
        .test
        .find_one(TestDocument::query().id1(doc0.id1))?
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
