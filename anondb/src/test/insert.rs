use super::*;

// #[derive(Document, Debug, Clone, PartialEq)]
// pub struct TestDocument<
//     'a,
//     const N: usize,
//     T: Default + Clone + PartialEq + std::fmt::Debug + Send + Sync,
// > {
//     pub id: u128,
//     pub other: String,
//     #[anondb(skip, default)]
//     pub v: Option<&'a T>,
// }
//
// impl<'a, const N: usize, T: Default + Clone + PartialEq + std::fmt::Debug + Send + Sync> Default
//     for TestDocument<'a, N, T>
// {
//     fn default() -> Self {
//         Self {
//             id: rand::random(),
//             other: rand_utf8(rand::random::<u16>().into()),
//             v: None,
//         }
//     }
// }
//
// #[tokio::test]
// async fn should_insert_document() -> Result<()> {
//     #[derive(AnonDB, Clone)]
//     pub struct DB<K: KV> {
//         #[anondb(primary_key = id)]
//         #[anondb(index = id, other; unique = true)]
//         pub test: Collection<TestDocument<'static, 1, u64>, K>,
//     }
//     let db = DB::<MemoryKV>::new(None).await?;
//     let doc = TestDocument::default();
//
//     assert_eq!(db.test.count().await?, 0);
//     db.test.insert(&doc).await?;
//     assert_eq!(db.test.count().await?, 1);
//
//     let found_doc = db
//         .test
//         .find_one(TestDocument::query())
//         .await?
//         .expect("expected to retrieve a document using empty query");
//     assert_eq!(
//         found_doc, doc,
//         "retrieved document mismatches inserted document"
//     );
//
//     Ok(())
// }
