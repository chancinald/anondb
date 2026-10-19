use super::*;

#[derive(Document, Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[tokio::test]
async fn should_update_document() -> Result<()> {
    #[derive(AnonDB, Clone)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = id)]
        #[anondb(index = id, other; unique = true)]
        #[anondb(index = other; unique = true)]
        pub test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    db.test
        .insert_many((0..50).map(|_| TestDocument::default()))
        .await?;
    let original_doc = TestDocument::default();
    db.test.insert(&original_doc).await?;
    db.test
        .insert_many((0..50).map(|_| TestDocument::default()))
        .await?;
    let start_count = db.test.count().await?;
    db.test
        .update(TestDocument::query(), |doc| {
            if doc.id == original_doc.id {
                let mut doc = doc.clone();
                doc.id = 1;
                Some(doc)
            } else {
                None
            }
        })
        .await?;
    assert_eq!(
        db.test.count().await?,
        start_count,
        "document count changed"
    );

    assert!(
        db.test
            .find_one(TestDocument::query().id(original_doc.id))
            .await?
            .is_none(),
        "original document should be removed"
    );

    let found_doc = db
        .test
        .find_one(TestDocument::query().id(1))
        .await?
        .expect("expected to retrieve a document with updated id");
    assert_eq!(
        found_doc.other, original_doc.other,
        "retrieved document mismatches inserted document"
    );

    Ok(())
}

#[derive(Document, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexedDocument {
    pub id: u64,
    pub tag: u64,
    pub name: String,
}

#[derive(AnonDB, Clone)]
pub struct IndexedDB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = tag)]
    #[anondb(index = name; unique = true)]
    pub docs: Collection<IndexedDocument, K>,
}

fn indexed(id: u64, tag: u64, name: &str) -> IndexedDocument {
    IndexedDocument {
        id,
        tag,
        name: name.into(),
    }
}

async fn tagged<K: KV>(db: &IndexedDB<K>, tag: u64) -> Result<Vec<u64>> {
    use futures::TryStreamExt;
    let mut ids = db
        .docs
        .find_many(IndexedDocument::query().tag(tag))
        .await?
        .map_ok(|doc| doc.id)
        .try_collect::<Vec<_>>()
        .await?;
    ids.sort();
    Ok(ids)
}

/// update_exact must replace the old document's index entries, not add to them.
#[tokio::test]
async fn update_exact_replaces_index_entries() -> Result<()> {
    async fn check<K: KV>(db: IndexedDB<K>) -> Result<()> {
        db.docs.insert(indexed(1, 7, "a")).await?;
        // unique field unchanged
        assert!(
            db.docs
                .query()
                .id(1)
                .update_exact(|mut d| {
                    d.tag = 8;
                    d
                })
                .await?
        );
        assert_eq!(tagged(&db, 7).await?, Vec::<u64>::new());
        assert_eq!(tagged(&db, 8).await?, vec![1]);
        // unique field changed: the old name is free again
        assert!(
            db.docs
                .query()
                .id(1)
                .update_exact(|mut d| {
                    d.name = "b".into();
                    d
                })
                .await?
        );
        db.docs.insert(indexed(2, 0, "a")).await?;
        assert!(
            db.docs.insert(indexed(3, 0, "b")).await.is_err(),
            "new name must be unique"
        );
        // primary key changed: the old document is gone
        assert!(
            db.docs
                .query()
                .id(1)
                .update_exact(|mut d| {
                    d.id = 5;
                    d
                })
                .await?
        );
        assert!(db.docs.query().id(1).find_exact().await?.is_none());
        assert_eq!(
            db.docs.query().id(5).find_exact().await?,
            Some(indexed(5, 8, "b"))
        );
        assert_eq!(db.docs.count().await?, 2);
        assert_eq!(tagged(&db, 8).await?, vec![5]);
        // an update colliding with another document's unique field fails and changes nothing
        assert!(
            db.docs
                .query()
                .id(5)
                .update_exact(|mut d| {
                    d.name = "a".into();
                    d
                })
                .await
                .is_err()
        );
        assert_eq!(
            db.docs.query().id(5).find_exact().await?,
            Some(indexed(5, 8, "b"))
        );
        Ok(())
    }
    check(IndexedDB::<MemoryKV>::new(None).await?).await?;
    check(IndexedDB::<RedbKV>::new(None).await?).await?;
    check(IndexedDB::<AnyKV>::new(None).await?).await?;
    check(IndexedDB::<AnyKV>::new(Some(temp_redb_config())).await?).await?;
    Ok(())
}

#[tokio::test]
async fn update_exact_or_insert_replaces_index_entries() -> Result<()> {
    async fn check<K: KV>(db: IndexedDB<K>) -> Result<()> {
        db.docs
            .query()
            .id(1)
            .update_exact_or_insert(|d| d.unwrap_or_else(|| indexed(1, 7, "a")))
            .await?;
        db.docs
            .query()
            .id(1)
            .update_exact_or_insert(|d| {
                let mut d = d.expect("document exists");
                d.tag = 9;
                d
            })
            .await?;
        assert_eq!(tagged(&db, 7).await?, Vec::<u64>::new());
        assert_eq!(tagged(&db, 9).await?, vec![1]);
        assert_eq!(db.docs.count().await?, 1);
        Ok(())
    }
    check(IndexedDB::<MemoryKV>::new(None).await?).await?;
    check(IndexedDB::<RedbKV>::new(None).await?).await?;
    check(IndexedDB::<AnyKV>::new(None).await?).await?;
    check(IndexedDB::<AnyKV>::new(Some(temp_redb_config())).await?).await?;
    Ok(())
}

fn temp_redb_config() -> AnyKVConfig {
    AnyKVConfig::Redb(Box::new(
        std::env::temp_dir().join(format!("db-{}.redb", rand::random::<u128>())),
    ))
}

#[tokio::test]
async fn any_kv_redb_config_persists() -> Result<()> {
    let config = temp_redb_config();
    let doc = indexed(1, 7, "a");
    {
        let db = IndexedDB::<AnyKV>::new(Some(config.clone())).await?;
        assert!(matches!(db.docs.kv(), AnyKV::Redb(_)));
        db.docs.insert(&doc).await?;
    }
    let db = IndexedDB::<AnyKV>::new(Some(config)).await?;
    assert_eq!(db.docs.query().id(1).find_exact().await?, Some(doc));
    assert_eq!(tagged(&db, 7).await?, vec![1]);
    Ok(())
}
