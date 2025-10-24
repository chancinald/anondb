use super::*;

#[test]
fn fail_insert_unique_index() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0)]
        #[anondb(index = id1; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc0 = TestDocument::default();
    let mut doc1 = TestDocument::default();
    doc1.id1 = doc0.id1;

    db.test.insert(&doc0)?;
    db.test
        .insert(&doc1)
        .expect_err("should fail to insert duplicate on index");

    Ok(())
}

#[test]
fn fail_insert_unique_compound_index_2() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0)]
        #[anondb(index = id1, id2; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc0 = TestDocument::default();
    let mut doc1 = TestDocument::default();
    doc1.id1 = doc0.id1;
    doc1.id2 = doc0.id2;

    db.test.insert(&doc0)?;
    db.test
        .insert(&doc1)
        .expect_err("should fail to insert duplicate on index");

    doc1.id2 = rand::random();
    db.test.insert(&doc1)?;

    Ok(())
}

#[test]
fn fail_insert_unique_compound_index_3() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0)]
        #[anondb(index = id1, id2, id3; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc0 = TestDocument::default();
    let mut doc1 = TestDocument::default();
    doc1.id1 = doc0.id1;
    doc1.id2 = doc0.id2;
    doc1.id3 = doc0.id3;

    db.test.insert(&doc0)?;
    db.test
        .insert(&doc1)
        .expect_err("should fail to insert duplicate on index");

    doc1.id2 = rand::random();
    db.test.insert(&doc1)?;

    Ok(())
}
