use super::*;

#[derive(Clone, Document, PartialEq, Debug)]
pub struct TestDocument {
    str_0: String,
    str_1: String,
    bytes: Vec<u8>,
    i: u32,
    u: u8,
    b: bool,
    bytes_maybe: Option<Vec<u8>>,
    str_maybe: Option<String>,
    slice: [u8; 10],
    r: u128,
}

impl Default for TestDocument {
    fn default() -> Self {
        Self {
            str_0: rand_utf8(5),
            str_1: rand_utf8(rand::random_range(2..100)),
            bytes: rand_bytes(rand::random_range(5..100)),
            i: rand::random(),
            u: rand::random(),
            b: rand::random(),
            r: rand::random(),
            bytes_maybe: if rand::random::<bool>() {
                Some(rand_bytes(rand::random_range(50..100)))
            } else {
                None
            },
            str_maybe: if rand::random::<bool>() {
                Some(rand_utf8(10))
            } else {
                None
            },
            slice: rand::random(),
        }
    }
}

#[tokio::test]
async fn should_find_exact() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    for _ in 0..20 {
        db.test.insert(&Default::default()).await?;
    }

    let doc = TestDocument::default();
    db.test.insert(&doc).await?;
    assert_eq!(
        doc,
        db.test
            .query()
            .u(doc.u)
            .r(doc.r)
            .find_exact()
            .await?
            .unwrap()
    );
    let write = db.write_tx().await?;
    assert_eq!(
        doc,
        write
            .test()
            .query()
            .u(doc.u)
            .r(doc.r)
            .find_exact()
            .await?
            .unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn find_exact_should_error_without_fields() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    assert!(
        db.test
            .query()
            .u(0)
            .find_exact()
            .await
            .expect_err("query should fail")
            .to_string()
            .starts_with("Query did not fully specify a primary key or unique index key")
    );
    let write = db.write_tx().await?;
    assert!(
        write
            .test()
            .query()
            .u(0)
            .find_exact()
            .await
            .expect_err("query should fail")
            .to_string()
            .starts_with(
                "Write transaction query did not fully specify a primary key or unique index key"
            )
    );
    Ok(())
}

#[tokio::test]
async fn update_exact_should_error_without_fields() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        #[anondb(index = r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    assert!({
        let s = db
            .test
            .query()
            .r(0)
            .update_exact(|d| d)
            .await
            .expect_err("query should fail")
            .to_string();
        println!("err: {s}");
        s.starts_with(
            "Write transaction query did not fully specify a primary key or unique index key",
        )
    });
    let write = db.write_tx().await?;
    assert!({
        let s = write
            .test()
            .query()
            .r(0)
            .update_exact(|d| d)
            .await
            .expect_err("query should fail")
            .to_string();
        println!("err: {s}");
        s.starts_with(
            "Write transaction query did not fully specify a primary key or unique index key",
        )
    });
    Ok(())
}

#[tokio::test]
async fn should_update_exact() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    for _ in 0..20 {
        db.test.insert(&Default::default()).await?;
    }

    {
        let doc = TestDocument::default();
        db.test.insert(&doc).await?;
        let new_r = rand::random();
        assert!(
            db.test
                .query()
                .u(doc.u)
                .r(doc.r)
                .update_exact(|mut found_doc| {
                    assert_eq!(found_doc, doc);
                    found_doc.r = new_r;
                    found_doc
                })
                .await?
        );
        db.test
            .query()
            .u(doc.u)
            .r(new_r)
            .find_exact()
            .await?
            .unwrap();
    }

    let doc = TestDocument::default();
    let new_r = rand::random();
    let write = db.write_tx().await?;
    write.test().insert(&doc).await?;
    assert!(
        write
            .test()
            .query()
            .u(doc.u)
            .r(doc.r)
            .update_exact(|mut found_doc| {
                assert_eq!(found_doc, doc);
                found_doc.r = new_r;
                found_doc
            })
            .await?
    );
    write
        .test()
        .query()
        .u(doc.u)
        .r(new_r)
        .find_exact()
        .await?
        .unwrap();
    assert!(
        db.test
            .query()
            .u(doc.u)
            .r(new_r)
            .find_exact()
            .await?
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn delete_exact_should_error_without_fields() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        #[anondb(index = r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    assert!(
        db.test
            .query()
            .r(0)
            .delete_exact()
            .await
            .expect_err("query should fail")
            .to_string()
            .starts_with(
                "Write transaction query did not fully specify a primary key or unique index key"
            )
    );
    let write = db.write_tx().await?;
    assert!(
        write
            .test()
            .query()
            .r(0)
            .delete_exact()
            .await
            .expect_err("query should fail")
            .to_string()
            .starts_with(
                "Write transaction query did not fully specify a primary key or unique index key"
            )
    );
    Ok(())
}

#[tokio::test]
async fn should_delete_exact() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    for _ in 0..20 {
        db.test.insert(&Default::default()).await?;
    }

    {
        let doc = TestDocument::default();
        db.test.insert(&doc).await?;
        assert_eq!(
            doc,
            // should return prev document
            db.test
                .query()
                .u(doc.u)
                .r(doc.r)
                .delete_exact()
                .await?
                .unwrap()
        );
        assert!(
            db.test
                .query()
                .u(doc.u)
                .r(doc.r)
                .find_exact()
                .await?
                .is_none()
        );
    }

    let doc = TestDocument::default();
    let write = db.write_tx().await?;
    write.test().insert(&doc).await?;
    assert_eq!(
        doc,
        // should return prev document
        write
            .test()
            .query()
            .u(doc.u)
            .r(doc.r)
            .delete_exact()
            .await?
            .unwrap()
    );
    assert!(
        write
            .test()
            .query()
            .u(doc.u)
            .r(doc.r)
            .find_exact()
            .await?
            .is_none()
    );
    Ok(())
}
