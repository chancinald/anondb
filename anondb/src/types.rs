mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed {
    /// Return a dynamic identifier for the type which
    /// is used for storing and comparing schema information
    fn type_name() -> &'static str;
}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {
            fn type_name() -> &'static str {
                stringify!($t)
            }
        }
    )* };
}
impl_native!(
    String, bool, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, f32, f64
);

impl<T: ValidDBType> sealed::Sealed for Option<T> {}
impl<T: ValidDBType> ValidDBType for Option<T> {
    fn type_name() -> &'static str {
        use std::sync::OnceLock;
        static NAME: OnceLock<String> = OnceLock::new();
        NAME.get_or_init(|| format!("Option<{}>", T::type_name()))
            .as_str()
    }
}

impl sealed::Sealed for Vec<u8> {}
impl ValidDBType for Vec<u8> {
    fn type_name() -> &'static str {
        "Vec<u8>"
        // use std::sync::OnceLock;
        // static NAME: OnceLock<String> = OnceLock::new();
        // NAME.get_or_init(|| format!("Vec<{}>", T::type_name()))
        //     .as_str()
    }
}

impl<T: ValidDBType, const N: usize> sealed::Sealed for [T; N] {}
impl<T: ValidDBType, const N: usize> ValidDBType for [T; N] {
    fn type_name() -> &'static str {
        use std::sync::OnceLock;
        static NAME: OnceLock<String> = OnceLock::new();
        NAME.get_or_init(|| format!("[{}; {}]", T::type_name(), N))
            .as_str()
    }
}
