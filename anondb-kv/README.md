## anondb-kv

This crate provides a set of traits for operating a key-value store.

### Assumptions

- All valid table names exist with 0 entries (opening a non-existent table never errors)
- Transactions provide a consistent view of the kv
