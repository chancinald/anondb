use std::collections::BTreeMap;

use futures::TryStreamExt;

use super::*;

#[derive(Clone, Document)]
pub struct TestDocument {
    str_0: String,
    str_1: String,
    bytes: Vec<u8>,
    i: u32,
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
            str_1: rand_utf8(rand::random_range(20..22)),
            bytes: rand_bytes(rand::random_range(50..100)),
            i: rand::random(),
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

macro_rules! test_permutation {
    ($($field:ident),+ $(,)?) => {
        paste::paste! {
            #[tokio::test]
            async fn [<order_compound_index $(_ $field)+>]() -> Result<()> {
                #[derive(AnonDB)]
                pub struct DB<K: KV> {
                    #[anondb(primary_key = $($field),+)]
                    test: Collection<TestDocument, K>,
                }

                let db = DB::<RedbKV>::new(None).await?;
                let mut tree = BTreeMap::new();

                for _ in 0..100 {
                    let doc = TestDocument::default();
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

                // let doc = TestDocument::default();
                // let tree_range_start = ($(doc.$field.clone(),)+);
                // let query = TestDocument::query()
                // $(
                //     . $field (ParamTyped::Range((doc.$field.clone() ..).into()))
                // )+ ;
                // let query = db.test.find_many(query).await?;
                // let stream = std::pin::pin!(query);
                // let docs = stream.try_collect::<Vec<_>>().await?;
                // for (doc, (key, _)) in docs.into_iter().zip(tree.range(tree_range_start..)) {
                //     let ($($field,)+) = key;
                //     $(
                //         assert_eq!(
                //             doc.$field.clone(), $field.clone(),
                //             concat!("mismatch on field `", stringify!($field), "`")
                //         );
                //     )+
                // }

                Ok(())
            }
        }
    };
}

// --- single field, sanity check each type in isolation ---
test_permutation!(str_0, r);
test_permutation!(bytes, r);
test_permutation!(i);
test_permutation!(b, r);
test_permutation!(bytes_maybe, r);
test_permutation!(str_maybe, r);
test_permutation!(slice);

// --- two fields, both orderings, mixing variable-length types ---
test_permutation!(str_0, str_1);
test_permutation!(str_1, str_0);
test_permutation!(bytes, slice);
test_permutation!(slice, bytes);
test_permutation!(str_0, bytes);
test_permutation!(bytes, str_0);

// --- Option fields combined with non-Option, both positions ---
test_permutation!(str_maybe, str_0);
test_permutation!(str_0, str_maybe);
test_permutation!(bytes_maybe, bytes);
test_permutation!(bytes, bytes_maybe);
test_permutation!(str_maybe, bytes_maybe, r);
test_permutation!(bytes_maybe, str_maybe, r);

// --- variable-length field NOT last
test_permutation!(str_0, str_1, i);
test_permutation!(bytes, str_0, i);
test_permutation!(str_maybe, str_0, slice);
test_permutation!(bytes_maybe, bytes, i);

// --- fixed-width types leading, variable-length trailing ---
test_permutation!(i, b, str_0);
test_permutation!(b, i, bytes);
test_permutation!(slice, i, str_1);

// --- full field set, a few different orderings ---
test_permutation!(str_0, str_1, bytes, i, b, bytes_maybe, str_maybe, slice);
test_permutation!(slice, str_maybe, bytes_maybe, b, i, bytes, str_1, str_0);
test_permutation!(i, b, slice, str_0, bytes, str_1, str_maybe, bytes_maybe);
test_permutation!(bytes_maybe, str_maybe, str_0, str_1, bytes, slice, i, b);
