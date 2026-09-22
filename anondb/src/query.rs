use super::*;

pub trait Query {
    /// Get the fields that are set in this query.
    fn fields(&self) -> Vec<(&'static str, Param)>;

    fn compute_scan_range(
        &self,
        index_field_names: &Vec<(String, LexStats)>,
    ) -> GeneralRange<Vec<u8>> {
        let fields = self.fields();
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
                Param::Eq(v) => {
                    min_key.append_key_slice(v);
                    max_key.append_key_slice(v);
                    min_bound = Bound::Included(min_key.to_vec());
                    max_bound = Bound::Included({
                        let mut v = max_key.clone();
                        v.append_upper_inclusive_byte();
                        v.into_vec()
                    });
                }
                Param::In(_v) => {
                    unimplemented!()
                }
                Param::Nin(_) | Param::Neq(_) => {
                    break;
                }
                Param::Range(v) => {
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
