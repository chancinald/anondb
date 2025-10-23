## AnonDB

An embedded document oriented NoSQL database implemented over a generic KV trait.

### MSRV

This crate targets Rust Edition 2024 with MSRV 1.85.

### Database

Each `database` contains collections of `documents`. Each `document` is `Serialize + Deserialize`. Each collection may specify `indices`. Each `index` specifies 1 or more field names from the `document`. Each `field` in an `index` must by a type that implements `SerializeLexicographic` (implementations are provided for most types).

Each `database` is generic over a trait `KV`, which abstracts a key-value store. The current implementation uses redb, with support for fjall being considered.

### Schema

The schema is statically defined and verifiable using structs, derive macros and field attributes.

```rs
use anondb::AnonDB;
use anondb::Document;
use anondb::KV;
use anondb::Collection;
use serde::Serialize;
use serde::Deserialize;

#[derive(Serialize, Deserialize, Document)]
pub struct User {
    pub id: u128,
    pub name: String,
    pub created_at: u64
}

#[derive(Serialize, Deserialize, Document)]
pub struct Post {
    pub id: u128,
    pub creator_id: u128,
    pub title: String,
    pub content: String,
    pub created_at: u64,
}

#[derive(AnonDB)]
pub struct DB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = name; unique = true)]
    #[anondb(index = created_at, name)]
    pub users: Collection<User, K>,
    #[anondb(primary_key = id)]
    #[anondb(index = creator_id, created_at)]
    pub posts: Collection<Post, K>,
}
```

In this example we see a database containing `User` and `Post` documents. We define primary keys for each, as well as indices over some fields.

- `#[anondb(primary_key = id)]` - each collection must have a primary key that is unique. This may be a compound key over multiple fields. Queries over this index returns the document data directly. Other indices retrieve document data via indirection (2 reads instead of 1).
- `#[anondb(index = name; unique)]` - constrains usernames to be unique. Inserting a document with a username that already exists will return an error.
- `#[anondb(index = name, created_at)]` - allows sortings and retrieval by username and creation date. This compound index will automatically be used to accelerate requests for the following pseudocode queries:

- `{ name = "username" }`
- `{ created_at > 19285889 }`
- `{ name.starts_with("bo") && created_at < 29485959 }`

In fact, the index on `name` is necessary only for the unique constraint. The compound index can serve most queries. Note that order matters in indices. For example, prefix matching a string works best if the string is later/last in the index. Additionally, filtering over `created_at` cannot be accelerated over an index `name, created_at`, but can be accelerated over `created_at, name`.

Schemas are statically analyzed at compile time. Indices can only be formed over types that implement `SerializeLexicographic`.

### Schema changes

At startup a description of the schema is automatically persisted into the database. When changes are made to document structs the system determines if changes are backward compatible. If changes are _not_ backward compatible, the system will refuse to start without a migration function.

The schema description includes information about indices. Indices are automatically created, deleted, and updated during normal database operation.

Indices are fully statically analyzable, so it's impossible to start the database with an index over a field that does not exist, or a field that cannot be serialized in a sortable way.

### Queries

The `Document` derive macro implements traits for statically analyzable queries.

```rs

#[derive(Serialize, Deserialize, Document)]
pub struct User {
    pub id: u128,
    pub name: String,
    pub created_at: u64
}

#[derive(AnonDB)]
pub struct DB<K: KV> {
    #[anondb(primary_key = id)]
    #[anondb(index = name; unique = true)]
    #[anondb(index = created_at, name)]
    pub users: Collection<User, K>,
}

let db = DB::<RedbKV>::in_memory(None)?;

let bob_user = User {
  id: rand::random(),
  name: "bob".into(),
  created_at: timestamp()
};

db.users.insert(&bob_user)?;

{
    let query = User::query().name("alice");
    let alice_maybe: Option<User> = db.users.find_one(query)?;
}

{
    let query = User::query().created_at(1761000295..1761200295);
    let recent_users: Vec<User> = db.users.find_many(query)?;
}

```

Each struct that derives `Document` has an associated function to build a query. This query has methods to set constraints for the query.

