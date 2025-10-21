use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::Range;

use anondb_kv::*;
use serde::Deserialize;
use serde::Serialize;

use super::*;

pub enum Param<'a, F>
where
    F: 'static + Serialize + for<'de> Deserialize<'de> + PartialEq,
{
    Eq(F),
    Neq(F),
    Range(Range<F>),
    /// Match values that are present in this array
    In(&'a [F]),
    /// Match values that are NOT present in this array
    Nin(&'a [F]),
}

impl<'a, F> Param<'a, F>
where
    F: 'static + Serialize + for<'de> Deserialize<'de> + PartialEq + PartialOrd,
{
    /// Test an instance of `T` against this parameter. Returns `true` if `T` matches self.
    pub fn test(&self, other: &F) -> bool {
        match self {
            Param::Eq(v) => v == other,
            Param::Range(v) => v.contains(other),
            Param::Neq(v) => v != other,
            Param::In(v) => v.contains(other),
            Param::Nin(v) => !v.contains(other),
        }
    }
}

/// A structure representing a query. We'll accept a selector function that will be executed during
/// scans, and a set of fields to determine what indices may be used to reduce the search space.
pub struct Query<T>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    selector: Option<fn(&T) -> bool>,
    _phantom: PhantomData<T>,
}

impl<T> Query<T>
where
    T: 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Given a set of fields with restrictions, determine a strategy for searching the database
    /// using all available indices.
    pub fn exec<K: KV>(&self, collection: &Collection<T, K>) {
        // start by iterating over the fields and looking for an index that fulfills

        // we're going to iterate over each parameter and refine the set of primary keys matching
        // the query as we go
        let mut primary_keys = HashSet::<&[u8]>::default();
    }
}

/// If there are any range parameters (gt, lt, etc) then a partial scan will be necessary?
///
/// When executing a query:
/// 1. Look for an index prefixed by all the fields being queried
/// 2. Look for fields that contain a unique index. Retrieve document(s) and run selector
/// 3. Look for an index prefixed by _some_ of the fields being queried. Scan said index
/// 4. Scan
#[macro_export]
macro_rules! find_one {
    ($collection:expr, $doctype:ty; $($field:ident: $param:expr),+) => {{
        use ::anondb_kv::Operations;

        // determines if a document matches the query
        fn selector(doc: &$doctype) -> bool {
            $(
                if !$param.test(&doc.$field) {
                    return false;
                }
            )+
            true
        }

        fn range_selector(key: &[u8], val: &[u8]) -> ::anyhow::Result<(Option<$doctype>, bool)> {

            Ok((None, false))
        }

        let mut fields = ::std::collections::HashMap::<String, ()>::default();
        $(
            if fields.contains_key(stringify!($field)) {
                ::anyhow::bail!("Query contains a duplicate key: \"{}\"", stringify!($field));
            }
            fields.insert(stringify!($field).into(), ());
        )+
        let field_names = fields.keys().cloned().collect::<Vec<_>>();

        for (_name, index) in $collection.indices() {
            if crate::is_index_prefix(&index.field_names, &field_names) {
                // use this index
            }
        }

        let _ = $collection.kv().range($collection.name(), .., ::anondb_kv::SortDirection::Asc, range_selector)?;
        let out: Option<$doctype> = None;
        out
    }};
}

/// Determine if an index is prefixed by the fields in a query
pub fn is_index_prefix(index_fields: &Vec<String>, query_fields: &Vec<String>) -> bool {
    if query_fields.len() > index_fields.len() {
        return false;
    }
    for index_field in &index_fields[..query_fields.len()] {
        if !query_fields.contains(index_field) {
            return false;
        }
    }
    true
}
