use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Bound;
use std::ops::RangeBounds;

use anondb_kv::*;
use serde::Deserialize;
use serde::Serialize;

use super::*;

pub enum Param {
    Eq(Vec<u8>),
    Neq(Vec<u8>),
    Range(KeyRange<Vec<u8>>),
    /// Match values that are present in this array
    In(Vec<Vec<u8>>),
    /// Match values that are NOT present in this array
    Nin(Vec<Vec<u8>>),
}

impl Param {
    pub fn eq<T: SerializeLexicographic>(val: &T) -> Self {
        Self::Eq(val.serialize_lex())
    }

    pub fn neq<T: SerializeLexicographic>(val: &T) -> Self {
        Self::Neq(val.serialize_lex())
    }

    pub fn range<T: SerializeLexicographic>(val: impl RangeBounds<T>) -> Self {
        Self::Range(KeyRange {
            start: match val.start_bound() {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(v) => Bound::Included(v.serialize_lex()),
                Bound::Excluded(v) => Bound::Excluded(v.serialize_lex()),
            },
            end: match val.end_bound() {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(v) => Bound::Included(v.serialize_lex()),
                Bound::Excluded(v) => Bound::Excluded(v.serialize_lex()),
            },
        })
    }

    pub fn inn<T: SerializeLexicographic>(val: Vec<&T>) -> Self {
        Self::In(val.into_iter().map(|v| v.serialize_lex()).collect())
    }

    pub fn nin<T: SerializeLexicographic>(val: Vec<&T>) -> Self {
        Self::Nin(val.into_iter().map(|v| v.serialize_lex()).collect())
    }
}

impl Param {
    /// Test an instance of `T` against this parameter. Returns `true` if `T` matches self.
    pub fn test(&self, other: &[u8]) -> bool {
        match self {
            Param::Eq(v) => v == other,
            Param::Range(v) => v.contains(&other.to_vec()),
            Param::Neq(v) => v != other,
            Param::In(v) => v.contains(&other.to_vec()),
            Param::Nin(v) => !v.contains(&other.to_vec()),
        }
    }
}

/// A structure representing a query.
///
/// A query is executed in two phases. First a set of documents is selected from the database using
/// indices if possible, or otherwise a simple scan. Next the documents are parsed into memory and filtered
/// using `selector`.
pub struct Query<T>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    pub field_names: HashMap<String, Param>,
    pub selector: fn(&T) -> bool,
}

/// If there are any range parameters (gt, lt, etc) then a partial scan will be necessary?
///
/// When executing a query:
/// 1. Look for an index prefixed by all the fields being queried
/// 2. Look for fields that contain a unique index. Retrieve document(s) and run selector
/// 3. Look for an index prefixed by _some_ of the fields being queried. Scan said index
/// 4. Scan
#[macro_export]
macro_rules! query {
    ($collection:expr, $doctype:ty; $($field:ident: $param:expr),+) => {{
        // determines if a document matches the query
        fn selector(doc: &$doctype) -> bool {
            $(
                let ser = <_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.$field);
                if !$param.test(&ser) {
                    return false;
                }
            )+
            true
        }

        let mut field_names = ::std::collections::HashMap::<String, crate::Param>::default();
        $(
            if field_names.contains_key(stringify!($field)) {
                ::anyhow::bail!("Query contains a duplicate key: \"{}\"", stringify!($field));
            }
            field_names.insert(stringify!($field).into(), $param);
        )+
        crate::Query {
            field_names,
            selector
        }
    }};
}
