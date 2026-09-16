use std::ops::Bound;

use anyhow::Result;
use redb::*;

pub(crate) fn to_redb_bounds<'a>(
    start: Bound<&'a Vec<u8>>,
    end: Bound<&'a Vec<u8>>,
) -> (Bound<&'a [u8]>, Bound<&'a [u8]>) {
    let map = |b: Bound<&'a Vec<u8>>| match b {
        Bound::Included(v) => Bound::Included(v.as_slice()),
        Bound::Excluded(v) => Bound::Excluded(v.as_slice()),
        Bound::Unbounded => Bound::Unbounded,
    };
    (map(start), map(end))
}

pub(crate) fn tabledef(name: &str) -> TableDefinition<'_, &'static [u8], &'static [u8]> {
    TableDefinition::new(name)
}

pub(crate) fn tabledef_multimap(
    name: &str,
) -> MultimapTableDefinition<'_, &'static [u8], &'static [u8]> {
    MultimapTableDefinition::new(name)
}

pub(crate) fn read_table(
    read: &ReadTransaction,
    name: &str,
) -> Result<Option<ReadOnlyTable<&'static [u8], &'static [u8]>>> {
    match read.open_table(tabledef(name)) {
        Ok(t) => Ok(Some(t)),
        Err(e) => {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                Ok(None)
            } else {
                Err(anyhow::anyhow!(e))
            }
        }
    }
}

pub(crate) fn read_multimap_table(
    read: &ReadTransaction,
    name: &str,
) -> Result<Option<ReadOnlyMultimapTable<&'static [u8], &'static [u8]>>> {
    match read.open_multimap_table(tabledef_multimap(name)) {
        Ok(t) => Ok(Some(t)),
        Err(e) => {
            if matches!(e, TableError::TableDoesNotExist(_)) {
                Ok(None)
            } else {
                Err(anyhow::anyhow!(e))
            }
        }
    }
}
