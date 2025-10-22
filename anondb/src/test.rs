use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TestDocument {
    pub id: u128,
    pub other: String,
}

#[derive(AnonDB)]
pub struct DB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = id, other; unique = true)]
    pub test_collection: Collection<TestDocument, K>,
}

#[test]
fn create_collections() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    db.test_collection.insert(&TestDocument {
        id: 0,
        other: "".into(),
    })?;

    assert_eq!(db.test_collection.count()?, 1);

    Ok(())
}

#[test]
fn should_fail_to_insert_duplicate_primary_key() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    let doc = TestDocument {
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
fn should_query_collection() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    let doc = TestDocument {
        id: 99,
        other: "".into(),
    };
    db.test_collection.insert(&doc)?;

    {
        let out = db
            .test_collection
            .find_one(query!(db.test_collection, TestDocument;
                id: Param::range(80u128..)
            ))?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }
    {
        let out = db
            .test_collection
            .find_one(query!(db.test_collection, TestDocument;
                id: Param::range(1000u128..)
            ))?;
        assert!(out.is_none());
    }
    {
        let out = db
            .test_collection
            .find_one(query!(db.test_collection, TestDocument;
                id: Param::eq(&99u128)
            ))?;
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out, doc);
    }

    Ok(())
}
