use futures::TryStreamExt;

use super::*;

#[tokio::test]
async fn should_insert_and_retrieve_documents() -> Result<()> {
    #[derive(AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id0)]
        #[anondb(index = id1, id2, id3, id4; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    for _ in 0..100 {
        db.test.insert(&TestDocument::default()).await?;
    }
    let query = db
        .test
        .find_many(TestDocument::query().id3(99..u128::MAX))
        .await?;
    let stream = std::pin::pin!(query);
    let docs = stream.try_collect::<Vec<_>>().await?;
    println!("found {}", docs.len());

    Ok(())
}
