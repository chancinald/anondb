use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CastTo {
    val: u128,
}

impl TryInto<CastTo> for u128 {
    type Error = ();
    fn try_into(self) -> std::result::Result<CastTo, Self::Error> {
        Ok(CastTo { val: self })
    }
}

impl Into<u128> for CastTo {
    fn into(self) -> u128 {
        self.val
    }
}

#[derive(Clone, Document, PartialEq, Debug)]
pub struct TestDocument {
    str_0: String,
    #[anondb(try_as = u128)]
    v: CastTo,
}

impl Default for TestDocument {
    fn default() -> Self {
        Self {
            str_0: rand_utf8(5),
            v: rand::random::<u128>().try_into().unwrap(),
        }
    }
}

#[tokio::test]
async fn should_use_casted_value_as_primary_key() -> Result<()> {
    #[derive(Clone, AnonDB)]
    pub struct DB<K: KV> {
        #[anondb(primary_key = v)]
        test: Collection<TestDocument, K>,
    }
    let db = DB::<MemoryKV>::new(None).await?;
    let doc = TestDocument::default();
    db.test.insert(&doc).await?;

    assert!(db.test.query().v(0u128).find_one().await?.is_none());
    assert_eq!(
        doc.clone(),
        db.test
            .query()
            .v(<CastTo as TryInto<u128>>::try_into(doc.v)?)
            .find_one()
            .await?
            .unwrap()
    );

    Ok(())
}
