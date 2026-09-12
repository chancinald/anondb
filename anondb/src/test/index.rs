use futures::TryStreamExt;

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

#[derive(AnonDB)]
pub struct DB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = id, other; unique = true)]
    pub test_collection: Collection<TestDocument2, K>,
    #[anondb(primary_key = id)]
    #[anondb(index = other)]
    pub other_collection: Collection<OtherDocument, K>,
}

#[tokio::test]
async fn should_use_non_unique_index() -> Result<()> {
    let db = DB::<MemoryKV>::new(None).await?;
    let doc = OtherDocument { id: 99, other: 99 };
    let doc2 = OtherDocument {
        id: 200,
        other: 200,
    };
    db.other_collection.insert(&doc).await?;
    db.other_collection.insert(&doc2).await?;

    let out = db
        .other_collection
        .find_many(OtherDocument::query().other(0..))
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    assert_eq!(out.len(), 2);
    assert_eq!(out.get(0).unwrap().other, 99);
    assert_eq!(out.get(1).unwrap().other, 200);

    Ok(())
}
