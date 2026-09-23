use super::*;

mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed + SerializeLexicographic + std::fmt::Debug {}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {}
    )* };
}
impl_native!(
    String, bool, /* i8, i16, i32, i64, i128,*/ u8, u16, u32, u64, u128 /* f32, f64*/
);

impl<T: ValidDBType + SerializeLexicographic + LexStats> sealed::Sealed for Option<T> {}
impl<T: ValidDBType + SerializeLexicographic + LexStats> ValidDBType for Option<T> {}

impl SerializeLexicographic for Box<dyn ValidDBType> {
    fn serialize_lex(&self) -> Vec<u8> {
        unimplemented!()
    }
}

// impl<T: ValidDBType> sealed::Sealed for Vec<T> {}
// impl<T: ValidDBType> ValidDBType for Vec<T> {}

impl sealed::Sealed for Vec<u8> {}
impl ValidDBType for Vec<u8> {}

impl<const N: usize> sealed::Sealed for [u8; N] {}
impl<const N: usize> ValidDBType for [u8; N] {}
