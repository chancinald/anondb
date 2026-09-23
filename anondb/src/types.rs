use super::*;

mod sealed {
    pub trait Sealed {}
}

/// A type that may be used in the database.
pub trait ValidDBType: sealed::Sealed + SerializeLexicographic + std::fmt::Debug {
    fn render(&self) -> String {
        format!("{self:?}")
    }
}

macro_rules! impl_native {
    ($($t:ty),*) => { $(
        impl sealed::Sealed for $t {}
        impl ValidDBType for $t {}
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
}

impl<T: ValidDBType + SerializeLexicographic + LexStats> sealed::Sealed for Option<T> {}
impl<T: ValidDBType + SerializeLexicographic + LexStats> ValidDBType for Option<T> {
    fn render(&self) -> String {
        if let Some(v) = self {
            v.render()
        } else {
            "None".into()
        }
    }
}

// impl<T: ValidDBType> sealed::Sealed for Vec<T> {}
// impl<T: ValidDBType> ValidDBType for Vec<T> {}

impl sealed::Sealed for Vec<u8> {}
impl ValidDBType for Vec<u8> {}

impl<const N: usize> sealed::Sealed for [u8; N] {}
impl<const N: usize> ValidDBType for [u8; N] {}
