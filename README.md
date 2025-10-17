## AnonDB

A document oriented database implemented over a generic KV trait.

### Database structure

Each database contains collections of documents. Each document is a `T: Serialize + Deserialize`. Each collection may specify indices. Each index specifies 1 or more field names in the document. Each field in an index must be one of `Bool`, `Integer`, `String`, or `Bytes`.


--

An AnonDB instance is a nosql document database implemented on top of a generic KV. The underlying KV must support tables with one value per key, tables with multiple values per key (multimap), atomic transactions, and lexicographic sorting of keys in tables.

We'll identify the higher level AnonDB instance as the "database" and the underlying data structure as the "KV". The database has "collections" of "documents", the KV has "tables" with "keys" and "values".

A database consists of a set of `Document: Serialize + Deserialize`. Databases encode documents as `&[u8]`.

Indices = function pointer that extracts field from a `Document` into a `Vec<u8>`. Indices are statically analyzable. Any field used in an index must be of a type that implements `SerializeLexicographic`.

Primary key = field on a `Document` that is used to identify the document in it's home table. This field must be of a type that implements `SerializeLexicographic`.

Collection = a homogenous set of documents in the database. Collections must contain a single document type. Queries may be executed over collections.


