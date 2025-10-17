use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use anondb_kv::*;
use anondb_macros::AnonDB;

use crate::*;

#[derive(Serialize, Deserialize)]
pub struct TestDocument {
    id: u128,
}

#[derive(AnonDB)]
pub struct DB<K: KV = RedbKV> {
    #[anondb(primary_key = id)]
    // #[anondb(index = id, unique)]
    pub test_collection: Collection<TestDocument, K>,
}

#[test]
fn create_collections() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    db.test_collection.insert(&TestDocument { id: 0 })?;

    assert_eq!(db.test_collection.count()?, 1);

    Ok(())
}

#[test]
fn should_fail_to_insert_duplicate_primary_key() -> Result<()> {
    let db = DB::<RedbKV>::in_memory(None)?;

    let doc = TestDocument { id: 99 };
    db.test_collection.insert(&doc)?;
    db.test_collection
        .insert(&doc)
        .expect_err("Should fail to insert duplicate primary key");

    Ok(())
}
