use std::borrow::Borrow;
use std::ops::Bound;
use std::ops::RangeBounds;

use anondb_kv::*;

#[derive(Debug)]
pub struct GeneralRange<T>(pub Bound<T>, pub Bound<T>);

impl GeneralRange<Vec<u8>> {
    pub fn as_slice<'a>(&'a self) -> GeneralRange<&'a [u8]> {
        GeneralRange(
            self.0.as_ref().map(|v| v.as_slice()),
            self.1.as_ref().map(|v| v.as_slice()),
        )
    }
}

impl<T: Clone> From<std::ops::Range<T>> for GeneralRange<T> {
    fn from(value: std::ops::Range<T>) -> Self {
        Self(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        )
    }
}
impl<T: Clone> From<std::ops::RangeFrom<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeFrom<T>) -> Self {
        Self(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        )
    }
}
impl<T: Clone> From<std::ops::RangeTo<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeTo<T>) -> Self {
        Self(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        )
    }
}

impl<T: Clone> From<std::ops::RangeInclusive<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeInclusive<T>) -> Self {
        Self(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        )
    }
}

impl<T: Clone> From<std::ops::RangeToInclusive<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeToInclusive<T>) -> Self {
        Self(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        )
    }
}

impl<T> RangeBounds<T> for GeneralRange<T> {
    fn start_bound(&self) -> Bound<&T> {
        self.0.as_ref()
    }
    fn end_bound(&self) -> Bound<&T> {
        self.1.as_ref()
    }
}

#[derive(Debug)]
pub enum ParamTyped<T: PartialEq + PartialOrd> {
    Eq(T),
    Neq(T),
    Range(GeneralRange<T>),
    /// Match values that are present in this array
    In(Vec<T>),
    /// Match values that are NOT present in this array
    Nin(Vec<T>),
}

impl<T: PartialEq + PartialOrd> ParamTyped<T> {
    pub fn test(&self, other: &T) -> bool {
        match self {
            ParamTyped::Eq(v) => v == other,
            ParamTyped::Range(v) => v.contains(other),
            ParamTyped::Neq(v) => v != other,
            ParamTyped::In(v) => v.contains(other),
            ParamTyped::Nin(v) => !v.contains(other),
        }
    }
}

impl<T: PartialEq + PartialOrd> ParamTyped<T> {
    pub fn typed(_v: &T, some: impl Into<Self>) -> Self {
        some.into()
    }
}

impl<T: SerializeLexicographic + PartialEq + PartialOrd> Into<Param> for ParamTyped<T> {
    fn into(self) -> Param {
        (&self).into()
    }
}

impl<T: SerializeLexicographic + PartialEq + PartialOrd> Into<Param> for &ParamTyped<T> {
    fn into(self) -> Param {
        match self {
            ParamTyped::Eq(v) => Param::Eq(v.serialize_lex()),
            ParamTyped::Neq(v) => Param::Neq(v.serialize_lex()),
            ParamTyped::Range(v) => Param::Range(GeneralRange(
                v.0.as_ref().map(|v| v.serialize_lex()),
                v.1.as_ref().map(|v| v.serialize_lex()),
            )),
            ParamTyped::In(v) => Param::In(v.into_iter().map(|v| v.serialize_lex()).collect()),
            ParamTyped::Nin(v) => Param::Nin(v.into_iter().map(|v| v.serialize_lex()).collect()),
        }
    }
}

#[derive(Debug)]
pub enum Param {
    Eq(Vec<u8>),
    Neq(Vec<u8>),
    Range(GeneralRange<Vec<u8>>),
    /// Match values that are present in this array
    In(Vec<Vec<u8>>),
    /// Match values that are NOT present in this array
    Nin(Vec<Vec<u8>>),
}

impl From<&str> for Param {
    fn from(value: &str) -> Self {
        Param::eq(value)
    }
}
impl From<&'static str> for ParamTyped<&str> {
    fn from(value: &'static str) -> Self {
        ParamTyped::Eq(value)
    }
}
impl From<&str> for ParamTyped<String> {
    fn from(value: &str) -> Self {
        ParamTyped::Eq(value.into())
    }
}

macro_rules! eq_syntax {
    ($($type:ty),+) => {
        $(
        impl From<&$type> for ParamTyped<$type> {
            fn from(value: &$type) -> Self {
                ParamTyped::Eq(value.clone())
            }
        }
        impl From<$type> for ParamTyped<$type> {
            fn from(value: $type) -> Self {
                ParamTyped::Eq(value)
            }
        }
        impl From<$type> for Param {
            fn from(value: $type) -> Self {
                Param::eq(value)
            }
        }
        impl From<&$type> for Param {
            fn from(value: &$type) -> Self {
                Param::eq(value)
            }
        }
        )+
    };
}
eq_syntax!(String, u8, u16, u32, u64, u128);

macro_rules! range_syntax {
    ($($type:ty),+) => {
        $(
        impl From<std::ops::Range<$type>> for Param {
            fn from(value: std::ops::Range<$type>) -> Self {
                Param::range(value)
            }
        }
        impl From<std::ops::RangeFrom<$type>> for Param {
            fn from(value: std::ops::RangeFrom<$type>) -> Self {
                Param::range(value)
            }
        }
        impl From<std::ops::RangeTo<$type>> for Param {
            fn from(value: std::ops::RangeTo<$type>) -> Self {
                Param::range(value)
            }
        }
        impl From<std::ops::RangeInclusive<$type>> for Param {
            fn from(value: std::ops::RangeInclusive<$type>) -> Self {
                Param::range(value)
            }
        }
        impl From<std::ops::RangeToInclusive<$type>> for Param {
            fn from(value: std::ops::RangeToInclusive<$type>) -> Self {
                Param::range(value)
            }
        }

        impl From<std::ops::Range<$type>> for ParamTyped<$type> {
            fn from(value: std::ops::Range<$type>) -> Self {
                ParamTyped::Range(value.into())
            }
        }
        impl From<std::ops::RangeFrom<$type>> for ParamTyped<$type> {
            fn from(value: std::ops::RangeFrom<$type>) -> Self {
                ParamTyped::Range(value.into())
            }
        }
        impl From<std::ops::RangeTo<$type>> for ParamTyped<$type> {
            fn from(value: std::ops::RangeTo<$type>) -> Self {
                ParamTyped::Range(value.into())
            }
        }
        impl From<std::ops::RangeInclusive<$type>> for ParamTyped<$type> {
            fn from(value: std::ops::RangeInclusive<$type>) -> Self {
                ParamTyped::Range(value.into())
            }
        }
        impl From<std::ops::RangeToInclusive<$type>> for ParamTyped<$type> {
            fn from(value: std::ops::RangeToInclusive<$type>) -> Self {
                ParamTyped::Range(value.into())
            }
        }
        )+
    };
}
range_syntax!(u8, u16, u32, u64, u128);

impl Param {
    pub fn eq<T: SerializeLexicographic>(val: T) -> Self {
        Self::Eq(val.borrow().serialize_lex())
    }

    pub fn neq<T: SerializeLexicographic>(val: T) -> Self {
        Self::Neq(val.serialize_lex())
    }

    pub fn range<T: SerializeLexicographic>(val: impl RangeBounds<T>) -> Self {
        Self::Range(GeneralRange(
            val.start_bound().map(|v| v.serialize_lex()),
            val.end_bound().map(|v| v.serialize_lex()),
        ))
    }

    pub fn inn<T: SerializeLexicographic>(val: Vec<T>) -> Self {
        Self::In(val.into_iter().map(|v| v.serialize_lex()).collect())
    }

    pub fn nin<T: SerializeLexicographic>(val: Vec<T>) -> Self {
        Self::Nin(val.into_iter().map(|v| v.serialize_lex()).collect())
    }
}

impl Param {
    /// Test an instance of `T` against this parameter. Returns `true` if `T` matches self.
    pub fn test(&self, other: &[u8]) -> bool {
        match self {
            Param::Eq(v) => v == other,
            Param::Range(v) => v.contains(&other.to_vec()),
            Param::Neq(v) => v != other,
            Param::In(v) => v.contains(&other.to_vec()),
            Param::Nin(v) => !v.contains(&other.to_vec()),
        }
    }
}
