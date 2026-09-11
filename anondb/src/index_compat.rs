use anyhow::Result;

use super::*;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum IndexQueryCompatibility {
    /// Query contains no fields of query
    None,
    /// Query identifies a set of documents in an index
    CompleteMulti,
    /// Query identifies a specific document(s) in a unique index
    CompleteUnique,
}

impl IndexQueryCompatibility {
    /// Determine how compatible this index is with a given query. A higher score indicates a
    /// faster query. An index that matches exactly returns a high score.
    pub fn new(
        field_names: &Vec<(String, LexStats)>,
        index_params: &Vec<(&str, Param)>,
    ) -> Result<Self> {
        let mut is_full_prefix = true; // are we able to utilize all of the fields in this index?
        let mut is_range_match = false;
        // let mut matched_any = false;
        for (i, (name, lex_stats)) in field_names.iter().enumerate() {
            // is this the final field in the index?
            let is_last_field = i == field_names.len() - 1;
            let Some((_, query_param)) = index_params.iter().find(|(pname, _)| pname == name)
            else {
                is_full_prefix = false;
                break;
            };
            // matched_any = true;
            match query_param {
                Param::Eq(_) | Param::In(_) => {
                    //
                }
                Param::Range(_) => {
                    is_range_match = true;
                    if !is_last_field {
                        // if we have a range on a field that is not the last field in the
                        // index we don't consider this a full prefix match, because we'll have
                        // to iterate over other fields as well
                        is_full_prefix = false;
                    }
                    break; // break when we encounter a field that would necessitate a scan
                }
                Param::Neq(_) | Param::Nin(_) => {
                    if !is_last_field {
                        is_full_prefix = false;
                    }
                    break;
                }
            }
            if lex_stats.fixed_width.is_none() {
                if !is_last_field {
                    is_full_prefix = false;
                }
                break;
            }
        }
        Ok(if is_full_prefix && !is_range_match {
            Self::CompleteUnique
        } else {
            // TODO
            Self::None
        })
    }
}
