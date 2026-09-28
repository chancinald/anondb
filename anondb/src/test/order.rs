use std::collections::BTreeMap;

use futures::TryStreamExt;

use super::*;

#[derive(Clone, Document)]
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
async fn prefix_scan_independent() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = u, r)]
        test: Collection<TestDocument, K>,
    }

    let db = DB::<MemoryKV>::new(None).await?;
    let mut tree = BTreeMap::new();

    for _ in 0..1024 {
        let doc = TestDocument::default();
        db.test.insert(&doc).await?;
        tree.insert((doc.u, doc.r), ());
    }

    assert!(
        db.test
            .find_one(TestDocument::query().u(..0))
            .await?
            .is_none()
    );

    for u in [rand::random(), rand::random(), 0, 255, 254, 1] {
        let query = TestDocument::query().u(u..);
        let query = db.test.find_many(query).await?;
        let stream = std::pin::pin!(query);
        let docs = stream.try_collect::<Vec<_>>().await?;
        let filtered_tree = tree.keys().filter(|k| k.0 >= u).collect::<Vec<_>>();
        assert_eq!(docs.len(), filtered_tree.len());
        for (doc, key) in docs.into_iter().zip(filtered_tree.into_iter()) {
            assert_eq!(doc.u, key.0, concat!("mismatch on field `u`"));
            assert_eq!(doc.r, key.1, concat!("mismatch on field `r`"));
        }

        let query = TestDocument::query().u(..u);
        let query = db.test.find_many(query).await?;
        let stream = std::pin::pin!(query);
        let docs = stream.try_collect::<Vec<_>>().await?;
        let filtered_tree = tree.keys().filter(|k| k.0 < u).collect::<Vec<_>>();
        assert_eq!(docs.len(), filtered_tree.len());
        for (doc, key) in docs.into_iter().zip(filtered_tree) {
            assert_eq!(doc.u, key.0, concat!("mismatch on field `u`"));
            assert_eq!(doc.r, key.1, concat!("mismatch on field `r`"));
        }
    }

    Ok(())
}

#[tokio::test]
async fn simple_order_range() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = i, r)]
        test: Collection<TestDocument, K>,
    }

    let db = DB::<RedbKV>::new(None).await?;
    let mut tree = BTreeMap::new();

    for _ in 0..100 {
        let doc = TestDocument::default();
        db.test.insert(&doc).await?;
        tree.insert((doc.i, doc.r), ());
    }

    let query = db.test.find_many(TestDocument::query()).await?;
    let stream = std::pin::pin!(query);
    let docs = stream.try_collect::<Vec<_>>().await?;
    for (doc, (key, _)) in docs.into_iter().zip(tree.clone().into_iter()) {
        let (i, r) = key;
        assert_eq!(doc.i, i, concat!("mismatch on field `i`"));
        assert_eq!(doc.r, r, concat!("mismatch on field `r`"));
    }

    let doc = TestDocument::default();
    let tree_range_start = (doc.i, doc.r);
    let query = TestDocument::query().i(doc.i..).r(doc.r..);
    let query = db.test.find_many(query).await?;
    let stream = std::pin::pin!(query);
    let docs = stream.try_collect::<Vec<_>>().await?;
    let filtered_tree = tree
        .keys()
        .filter(|k| k.0 >= tree_range_start.0 && k.1 >= tree_range_start.1)
        .collect::<Vec<_>>();
    assert_eq!(filtered_tree.len(), docs.len());
    // we cannot use plain lexicographic sorting of the BTreeMap because
    // querying the indices separately is different than querying a lexicographic
    // range. Instead we're querying a rectangular space formed over the fields independently
    for (doc, key) in docs.into_iter().zip(filtered_tree.into_iter()) {
        let (i, r) = key;
        assert_eq!(doc.i, *i, concat!("mismatch on field `i`"));
        assert_eq!(doc.r, *r, concat!("mismatch on field `r`"));
    }

    let query = TestDocument::query().i(..doc.i).r(..doc.r);
    let query = db.test.find_many(query).await?;
    let stream = std::pin::pin!(query);
    let docs = stream.try_collect::<Vec<_>>().await?;
    let filtered_tree = tree
        .keys()
        .filter(|k| k.0 < tree_range_start.0 && k.1 < tree_range_start.1)
        .collect::<Vec<_>>();

    assert_eq!(filtered_tree.len(), docs.len());
    for (doc, key) in docs.into_iter().zip(filtered_tree.into_iter()) {
        let (i, r) = key;
        assert_eq!(doc.i, *i, concat!("mismatch on field `i`"));
        assert_eq!(doc.r, *r, concat!("mismatch on field `r`"));
    }

    Ok(())
}

macro_rules! test_permutation {
    ($($field:ident),+ $(,)?) => {
        paste::paste! {
            #[tokio::test]
            async fn [<order_compound_index $(_ $field)+>]() -> Result<()> {
                #[derive(Clone, AnonDB)]
                pub struct DB<K: KV> {
                    #[anondb(primary_key = $($field),+)]
                    test: Collection<TestDocument, K>,
                }

                let db = DB::<RedbKV>::new(None).await?;
                let mut tree = BTreeMap::new();

                for _ in 0..100 {
                    let doc = TestDocument::default();
                    {
                        let mut rdoc = TestDocument::default();
                        rdoc.bytes = vec![doc.bytes.clone(), vec![0], doc.bytes.clone()].concat();
                        rdoc.str_1 = format!("{}{}", &doc.str_1, &rdoc.str_1);
                        rdoc.b = doc.b;
                        db.test.insert(&rdoc).await?;
                        tree.insert(($(rdoc.$field.clone(),)+), ());
                    }
                    db.test.insert(&doc).await?;
                    tree.insert(($(doc.$field.clone(),)+), ());
                }

                let query = db.test.find_many(TestDocument::query()).await?;
                let stream = std::pin::pin!(query);
                let docs = stream.try_collect::<Vec<_>>().await?;

                for (doc, (key, _)) in docs.into_iter().zip(tree.clone().into_iter()) {
                    let ($($field,)+) = key;
                    $(
                        assert_eq!(
                            doc.$field, $field,
                            concat!("mismatch on field `", stringify!($field), "`")
                        );
                    )+
                }

                let mut range_start;
                loop {
                range_start = TestDocument::default();
                let query = TestDocument::query()
                $(
                    . $field (range_start.$field.clone() ..)
                )+ ;
                let query = db.test.find_many(query).await?;
                let stream = std::pin::pin!(query);
                let docs = stream.try_collect::<Vec<_>>().await?;
                let filtered_tree = tree.keys().filter(|k| {
                    let ($($field,)+) = k;
                    $(
                        $field >= &range_start.$field
                    )&&*
                }).collect::<Vec<_>>();
                assert_eq!(docs.len(), filtered_tree.len());
                    let range_start_maybe = docs.first().cloned();
                for (doc, key) in docs.into_iter().zip(filtered_tree.into_iter()) {
                    let ($($field,)+) = key;
                    $(
                        assert_eq!(
                            doc.$field.clone(), $field.clone(),
                            concat!("mismatch on field `", stringify!($field), "`")
                        );
                    )+
                }
                if let Some(start) = range_start_maybe {
                    range_start = start;
                    break;
                }
                }

                {
                let query = TestDocument::query()
                $(
                    . $field (range_start.$field.clone() ..)
                )+ ;
                let query = db.test.find_many(query).await?;
                let stream = std::pin::pin!(query);
                let docs = stream.try_collect::<Vec<_>>().await?;
                let filtered_tree = tree.keys().filter(|k| {
                    let ($($field,)+) = k;
                    $(
                        $field >= &range_start.$field
                    )&&*
                }).collect::<Vec<_>>();
                assert_eq!(docs.len(), filtered_tree.len());
                for (doc, key) in docs.into_iter().zip(filtered_tree.into_iter()) {
                    let ($($field,)+) = key;
                    $(
                        assert_eq!(
                            doc.$field.clone(), $field.clone(),
                            concat!("mismatch on field `", stringify!($field), "`")
                        );
                    )+
                }
                }



                Ok(())
            }
        }
    };
}

// --- single field, sanity check each type in isolation ---
test_permutation!(r, str_0);
test_permutation!(r, bytes);
test_permutation!(i);
test_permutation!(b, r);
test_permutation!(r, bytes_maybe);
test_permutation!(r, str_maybe);
test_permutation!(slice);

// --- fixed-width types leading, variable-length trailing ---
test_permutation!(i, b, str_0);
test_permutation!(b, i, bytes);
test_permutation!(slice, i, str_1);
test_permutation!(b, str_1);
