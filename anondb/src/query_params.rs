use std::ops::Bound;
use std::ops::RangeBounds;

use super::*;

#[derive(Debug, Clone)]
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
        Self(value.start_bound().cloned(), value.end_bound().cloned())
    }
}
impl<T: Clone> From<std::ops::RangeFrom<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeFrom<T>) -> Self {
        Self(value.start_bound().cloned(), value.end_bound().cloned())
    }
}
impl<T: Clone> From<std::ops::RangeTo<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeTo<T>) -> Self {
        Self(value.start_bound().cloned(), value.end_bound().cloned())
    }
}

impl<T: Clone> From<std::ops::RangeInclusive<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeInclusive<T>) -> Self {
        Self(value.start_bound().cloned(), value.end_bound().cloned())
    }
}

impl<T: Clone> From<std::ops::RangeToInclusive<T>> for GeneralRange<T> {
    fn from(value: std::ops::RangeToInclusive<T>) -> Self {
        Self(value.start_bound().cloned(), value.end_bound().cloned())
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

#[derive(Debug, Clone)]
pub enum QueryParam<F: ValidDBType> {
    Eq(F),
    Range(GeneralRange<F>),
}

impl From<&str> for QueryParam<String> {
    fn from(value: &str) -> Self {
        QueryParam::Eq(value.to_string())
    }
}

macro_rules! eq_syntax {
    ($($type:ty),+) => {
        $(
        impl From<$type> for QueryParam<$type> {
            fn from(value: $type) -> Self {
                QueryParam::Eq(value)
            }
        }
        impl From<&$type> for QueryParam<$type> {
            fn from(value: &$type) -> Self {
                QueryParam::Eq(value.clone())
            }
        }
        )+
    };
}
eq_syntax!(String, u8, u16, u32, u64, u128);

impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::Range<F>> for QueryParam<F> {
    fn from(value: ::std::ops::Range<F>) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        ))
    }
}
impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::RangeInclusive<F>>
    for QueryParam<F>
{
    fn from(value: ::std::ops::RangeInclusive<F>) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        ))
    }
}
impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::RangeFrom<F>>
    for QueryParam<F>
{
    fn from(value: ::std::ops::RangeFrom<F>) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        ))
    }
}
impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::RangeTo<F>>
    for QueryParam<F>
{
    fn from(value: ::std::ops::RangeTo<F>) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        ))
    }
}
impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::RangeToInclusive<F>>
    for QueryParam<F>
{
    fn from(value: ::std::ops::RangeToInclusive<F>) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v| v.clone()),
            value.end_bound().map(|v| v.clone()),
        ))
    }
}
impl<F: ValidDBType + PartialEq + PartialOrd + Clone> From<::std::ops::RangeFull>
    for QueryParam<F>
{
    fn from(value: ::std::ops::RangeFull) -> Self {
        Self::Range(GeneralRange(
            value.start_bound().map(|v: &F| v.clone()),
            value.end_bound().map(|v: &F| v.clone()),
        ))
    }
}

impl<F: ValidDBType + PartialEq + PartialOrd + Clone> QueryParam<F> {
    /// Test an instance of `T` against this parameter. Returns `true` if `T` matches self.
    pub fn test(&self, val: &F) -> bool {
        match self {
            QueryParam::Eq(v) => v == val,
            QueryParam::Range(v) => v.contains(val),
        }
    }

    pub fn serialize(&self) -> QueryParam<Vec<u8>> {
        match self {
            Self::Eq(v) => QueryParam::Eq(v.serialize_lex()),
            Self::Range(v) => QueryParam::Range(GeneralRange(
                v.start_bound().map(|v| v.serialize_lex()),
                v.end_bound().map(|v| v.serialize_lex()),
            )),
        }
    }
}
