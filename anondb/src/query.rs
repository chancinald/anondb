use std::ops::RangeBounds;

use super::*;

pub trait GeneralQueryBuilder<Q, F: ValidDBType> {
    fn with_query(query: Q, on_param: impl (Fn(QueryParam<F>, Q) -> Q) + 'static) -> Self;
    fn eq(self, v: F) -> Q;
    fn range(self, v: impl RangeBounds<F>) -> Q;
}

pub struct QueryBuilder<Q, F: ValidDBType> {
    query: Q,
    on_param: Box<dyn Fn(QueryParam<F>, Q) -> Q>,
}

impl<Q, F: ValidDBType + Clone> GeneralQueryBuilder<Q, F> for QueryBuilder<Q, F> {
    fn with_query(query: Q, on_param: impl (Fn(QueryParam<F>, Q) -> Q) + 'static) -> Self {
        Self {
            query,
            on_param: Box::new(on_param),
        }
    }

    fn eq(self, v: F) -> Q {
        (self.on_param)(QueryParam::Eq(v), self.query)
    }

    fn range(self, v: impl RangeBounds<F>) -> Q {
        (self.on_param)(
            QueryParam::Range(GeneralRange(
                v.start_bound().map(|v| v.clone()),
                v.end_bound().map(|v| v.clone()),
            )),
            self.query,
        )
    }
}

pub trait Query {
    /// Get the fields that are set in this query.
    fn serialize(&self) -> Vec<(&'static str, QueryParam<Vec<u8>>)>;

    fn compute_scan_range(
        &self,
        index_field_names: &Vec<(String, OwnedLexStats)>,
    ) -> GeneralRange<Vec<u8>> {
        let fields = self.serialize();
        use std::ops::Bound;
        use std::ops::RangeBounds;
        let mut min_key = LexicographicKey::default();
        let mut max_key = LexicographicKey::default();
        let mut min_bound: Bound<Vec<u8>> = Bound::Unbounded;
        let mut max_bound: Bound<Vec<u8>> = Bound::Unbounded;
        for (name, lex_stats) in index_field_names {
            let Some((_, query_param)) = fields.iter().find(|(pname, _)| pname == name) else {
                // the query isn't using this field of the index. If this field is constant width
                // we can continue attempting to use the index.
                if let Some(width) = lex_stats.fixed_width {
                    let min = vec![0u8; width as usize];
                    let max = vec![u8::MAX; width as usize];
                    min_key.append_key_slice(&min);
                    max_key.append_key_slice(&max);
                    min_bound = Bound::Included(min_key.to_vec());
                    max_bound = Bound::Included({
                        let mut v = max_key.clone();
                        v.append_upper_inclusive_byte();
                        v.into_vec()
                    });
                    continue;
                }

                // otherwise we have to halt and begin scanning
                break;
            };
            match query_param {
                QueryParam::Eq(v) => {
                    min_key.append_key_slice(v);
                    max_key.append_key_slice(v);
                    min_bound = Bound::Included(min_key.to_vec());
                    max_bound = Bound::Included({
                        let mut v = max_key.clone();
                        v.append_upper_inclusive_byte();
                        v.into_vec()
                    });
                }
                QueryParam::Range(v) => {
                    match v.start_bound() {
                        Bound::Unbounded => {}
                        Bound::Included(v) | Bound::Excluded(v) => {
                            // we always treat it as included to account for earlier fields
                            // that may exist in the key
                            min_key.append_key_slice(&v);
                            min_bound = Bound::Included(min_key.into_vec());
                        }
                    }
                    match v.end_bound() {
                        Bound::Unbounded => {
                            max_bound = Bound::Unbounded;
                        }
                        Bound::Included(v) | Bound::Excluded(v) => {
                            max_key.append_key_slice(&v);
                            max_key.append_upper_inclusive_byte();
                            max_bound = Bound::Included(max_key.into_vec());
                        }
                    }
                    break;
                }
            }
        }
        GeneralRange(min_bound, max_bound)
    }
}
