use super::*;

mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed + SerializeLexicographic + std::fmt::Debug + 'static {
    fn render(&self) -> String {
        format!("{self:?}")
    }

    fn schema_type_id() -> &'static str
    where
        Self: Sized;
}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {
            fn schema_type_id() -> &'static str {
                schema_id::<Self>(stringify!($t))
            }
        }
    )* };
}
impl_native!(
    bool, /* i8, i16, i32, i64, i128,*/ u8, u16, u32, u64, u128 /* f32, f64*/
);

impl sealed::Sealed for String {}
impl ValidDBType for String {
    fn render(&self) -> String {
        self.clone()
    }

    fn schema_type_id() -> &'static str {
        schema_id::<Self>("String")
    }
}

impl sealed::Sealed for Vec<u8> {}
impl ValidDBType for Vec<u8> {
    fn schema_type_id() -> &'static str {
        schema_id::<Self>("Vec<u8>")
    }
}

macro_rules! string_cache {
    () => {
        fn string_cache_once<T: 'static>(f: fn() -> &'static str) -> &'static str {
            use std::any::TypeId;
            use std::sync::LazyLock;
            use std::sync::RwLock;
            static CACHE: LazyLock<RwLock<HashMap<TypeId, &'static str>>> =
                LazyLock::new(|| Default::default());
            let type_id = TypeId::of::<T>();
            if let Some(&v) = CACHE.read().unwrap().get(&type_id) {
                v
            } else {
                let mut write = CACHE.write().unwrap();
                if let Some(&v) = write.get(&type_id) {
                    return v;
                }
                let v = f();
                assert!(write.insert(type_id, v).is_none());
                v
            }
        }
    };
}

impl<T: ValidDBType + LexStats> sealed::Sealed for Option<T> {}
impl<T: ValidDBType + LexStats> ValidDBType for Option<T> {
    fn render(&self) -> String {
        if let Some(v) = self {
            v.render()
        } else {
            "None".into()
        }
    }

    fn schema_type_id() -> &'static str {
        string_cache!();
        let id = string_cache_once::<Self>(|| {
            Box::leak(format!("Option<{}>", T::schema_type_id()).into_boxed_str())
        });
        schema_id::<Self>(id)
    }
}

// impl<T: ValidDBType> sealed::Sealed for Vec<T> {}
// impl<T: ValidDBType> ValidDBType for Vec<T> {}

impl<const N: usize> sealed::Sealed for [u8; N] {}
impl<const N: usize> ValidDBType for [u8; N] {
    fn schema_type_id() -> &'static str {
        string_cache!();
        let id = string_cache_once::<Self>(|| Box::leak(format!("[u8; {N}]").into_boxed_str()));
        schema_id::<Self>(id)
    }
}

fn schema_id<T: 'static>(id: &'static str) -> &'static str {
    #[cfg(debug_assertions)]
    {
        use std::any::TypeId;
        use std::sync::LazyLock;
        use std::sync::RwLock;
        static TYPE_ID_SCHEMA_ID_MAP: LazyLock<RwLock<HashMap<TypeId, &'static str>>> =
            LazyLock::new(|| RwLock::new(Default::default()));
        let type_id = TypeId::of::<T>();
        if let Some(&v) = TYPE_ID_SCHEMA_ID_MAP.read().unwrap().get(&type_id) {
            assert_eq!(v, id);
            v
        } else {
            let mut write = TYPE_ID_SCHEMA_ID_MAP.write().unwrap();
            if let Some(&v) = write.get(&type_id) {
                assert_eq!(v, id);
                return v;
            }
            for &v in write.values() {
                assert_ne!(v, id);
            }
            assert!(write.insert(type_id, id).is_none());
            id
        }
    }
    #[cfg(not(debug_assertions))]
    id
}
