use super::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Document)]
pub struct TestDocument2 {
    pub id: u128,
    pub other: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Document)]
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
    pub other_collection: Collection<OtherDocument, K>,
}

#[test]
fn create_collections() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    db.test_collection.insert(&TestDocument2 {
        id: 0,
        other: "".into(),
    })?;

    assert_eq!(db.test_collection.count()?, 1);

    Ok(())
}

#[test]
fn should_fail_to_insert_duplicate_primary_key() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    let doc = TestDocument2 {
        id: 99,
        other: "".into(),
    };
    db.test_collection.insert(&doc)?;
    db.test_collection
        .insert(&doc)
        .expect_err("Should fail to insert duplicate primary key");

    Ok(())
}

#[test]
fn should_use_non_unique_index() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;
    let doc = OtherDocument { id: 99, other: 99 };
    let doc2 = OtherDocument {
        id: 200,
        other: 200,
    };
    db.other_collection.insert(&doc)?;
    db.other_collection.insert(&doc2)?;

    let out = db
        .other_collection
        .find_many(OtherDocument::query().other(0..))?
        .collect::<Vec<_>>();
    assert_eq!(out.len(), 2);
    assert_eq!(out.get(0).unwrap().other, 99);
    assert_eq!(out.get(1).unwrap().other, 200);

    Ok(())
}

#[test]
fn should_query_collection() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    let doc = TestDocument2 {
        id: 99,
        other: "".into(),
    };
    db.test_collection.insert(&doc)?;

    {
        let out = db
            .test_collection
            .find_one(TestDocument2::query().id(80..).other(""))?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }
    {
        let out = db
            .test_collection
            .find_one(TestDocument2::query().id(1000..))?;
        assert!(out.is_none());
    }
    {
        let out = db.test_collection.find_one(TestDocument2::query().id(99))?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }

    Ok(())
}
