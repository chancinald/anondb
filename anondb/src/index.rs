use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq)]
pub struct Index<T> {
    pub name: String,
    pub field_names: Vec<String>,
    pub serialize: fn(&T) -> Vec<u8>,
    pub options: IndexOptions,
    pub _phantom: PhantomData<T>,
}

impl<T> Index<T> {
    /// Name of the table in the kv where the index will be stored. This should be a combination of
    /// the collection name and the fields being indexed.
    pub fn id(&self) -> String {
        format!(
            "index_{}{}",
            self.name,
            if self.options.unique { "_unique" } else { "" }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Default)]
pub struct IndexOptions {
    pub unique: bool, // only allow 1 unique combination of each field in the index
}

#[macro_export]
macro_rules! primary_key {
    ($struct_name:ty, $field:ident) => {
        (stringify!($field).into(), |doc: &$struct_name| -> Vec<u8> {
            <_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.$field)
        })
    };
}

#[macro_export]
macro_rules! index {
    ($struct_name:ty, $($field:ident),+) => {
        crate::index!($struct_name, $($field),+ ; crate::IndexOptions::default())
    };

    ($struct_name:ty, $($field:ident),+ ; $options:expr) => {{
        crate::Index {
            name: vec![std::any::type_name::<$struct_name>(), $(stringify!($field)),+].join("_"),
            field_names: vec![$(stringify!($field).to_string()),+],
            serialize: |doc: &$struct_name| -> Vec<u8> {
                let mut bytes = Vec::default();
                $(
                    // This line enforces the trait bound at compile time.
                    // If $field doesn't implement SerializeLexicographic,
                    // this won't compile
                    bytes.extend(<_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.$field));
                )+
                bytes
            },
            options: $options,
            _phantom: std::marker::PhantomData::default()
        }
    }};
}
