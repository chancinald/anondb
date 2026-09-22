mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed {}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {}
    )* };
}
impl_native!(
    String, bool, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, f32, f64
);

impl<T: ValidDBType> sealed::Sealed for Option<T> {}
impl<T: ValidDBType> ValidDBType for Option<T> {}

impl sealed::Sealed for Vec<u8> {}
impl ValidDBType for Vec<u8> {}

impl<T: ValidDBType, const N: usize> sealed::Sealed for [T; N] {}
impl<T: ValidDBType, const N: usize> ValidDBType for [T; N] {}
