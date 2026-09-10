mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed {
    /// Return a dynamic identifier for the type which
    /// is used for storing and comparing schema information
    fn type_name() -> String;
}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {
            fn type_name() -> String {
                stringify!($t).into()
            }
        }
    )* };
}
impl_native!(
    String, bool, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, f32, f64
);

impl<T: ValidDBType> sealed::Sealed for Option<T> {}
impl<T: ValidDBType> ValidDBType for Option<T> {
    fn type_name() -> String {
        format!("Option<{}>", T::type_name())
    }
}
impl<T: ValidDBType> sealed::Sealed for Vec<T> {}
impl<T: ValidDBType> ValidDBType for Vec<T> {
    fn type_name() -> String {
        format!("Vec<{}>", T::type_name())
    }
}

impl<T: ValidDBType, const N: usize> sealed::Sealed for [T; N] {}
impl<T: ValidDBType, const N: usize> ValidDBType for [T; N] {
    fn type_name() -> String {
        format!("[{}; {}]", T::type_name(), N)
    }
}
