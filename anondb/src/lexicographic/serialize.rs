use std::marker::PhantomData;

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OwnedLexStats {
    pub min: Vec<u8>,
    pub max: Option<Vec<u8>>,
    pub fixed_width: Option<u32>,
}

/// Allow a type to be serialized to lexicographically comparable bytes
pub trait LexStats {
    fn min() -> Vec<u8>;
    fn max() -> Option<Vec<u8>> {
        None
    }
    fn fixed_width() -> Option<u32> {
        None
    }
    fn to_owned() -> OwnedLexStats {
        OwnedLexStats {
            min: Self::min(),
            max: Self::max(),
            fixed_width: Self::fixed_width(),
        }
    }
}
pub trait SerializeLexicographic {
    fn serialize_lex(&self) -> Vec<u8>;
}

pub trait DeserializeLexicographic: SerializeLexicographic + LexStats + Sized {
    fn deserialize(bytes: &[u8], len_maybe: Option<usize>) -> Result<(Self, &[u8])> {
        let len = match (Self::fixed_width(), len_maybe) {
            (Some(_), Some(_)) => {
                anyhow::bail!("cannot provide explicit length for fixed size element")
            }
            (None, None) => {
                anyhow::bail!("must provide explicit length for fixed size element")
            }
            (Some(len), None) => len as usize,
            (None, Some(len)) => len,
        };
        if bytes.len() < len {
            anyhow::bail!("bad SerializeLexicographic byte length")
        }
        let (bytes, out) = bytes.split_at(len);
        Ok((Self::from_bytes(bytes)?, out))
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self>;
}

impl<T: SerializeLexicographic + LexStats> SerializeLexicographic for Option<T> {
    fn serialize_lex(&self) -> Vec<u8> {
        match self {
            Some(v) => vec![vec![0x01], SerializeLexicographic::serialize_lex(v)].concat(),
            None => vec![0x00],
        }
    }
}

impl<T: LexStats> LexStats for Option<T> {
    fn min() -> Vec<u8> {
        vec![0x00]
    }

    fn max() -> Option<Vec<u8>> {
        T::max().map(|v| vec![vec![0x01], v].concat())
    }
}

impl<T: DeserializeLexicographic> DeserializeLexicographic for Option<T> {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        assert!(!bytes.is_empty());
        match bytes[0] {
            0 => Ok(Self::None),
            1 => Ok(Self::Some(T::from_bytes(&bytes[1..])?)),
            _ => {
                anyhow::bail!("bad option first byte")
            }
        }
    }
}

impl SerializeLexicographic for bool {
    fn serialize_lex(&self) -> Vec<u8> {
        if *self { vec![0x01] } else { vec![0x00] }
    }
}

impl LexStats for bool {
    fn min() -> Vec<u8> {
        vec![0x00]
    }

    fn max() -> Option<Vec<u8>> {
        Some(vec![0x01])
    }

    fn fixed_width() -> Option<u32> {
        Some(1)
    }
}

impl DeserializeLexicographic for bool {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(match bytes[0] {
            0 => false,
            1 => true,
            _ => anyhow::bail!("bad byte for bool: {}", bytes[0]),
        })
    }
}

impl SerializeLexicographic for Vec<u8> {
    fn serialize_lex(&self) -> Vec<u8> {
        self.clone()
    }
}

impl LexStats for Vec<u8> {
    fn min() -> Vec<u8> {
        vec![]
    }
}

impl DeserializeLexicographic for Vec<u8> {
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(bytes.to_vec())
    }
}

impl SerializeLexicographic for String {
    fn serialize_lex(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl LexStats for String {
    fn min() -> Vec<u8> {
        vec![]
    }
}

impl DeserializeLexicographic for String {
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self::from_utf8(bytes.to_vec())?)
    }
}

impl SerializeLexicographic for &str {
    fn serialize_lex(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl LexStats for &str {
    fn min() -> Vec<u8> {
        vec![]
    }
}

impl SerializeLexicographic for &String {
    fn serialize_lex(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl LexStats for &String {
    fn min() -> Vec<u8> {
        vec![]
    }
}

macro_rules! lex_uint {
    ($int:ident) => {
        impl SerializeLexicographic for &$int {
            fn serialize_lex(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }
        }

        impl LexStats for &$int {
            fn min() -> Vec<u8> {
                $int::MIN.to_be_bytes().to_vec()
            }

            fn max() -> Option<Vec<u8>> {
                Some($int::MAX.to_be_bytes().to_vec())
            }

            fn fixed_width() -> Option<u32> {
                assert_eq!(
                    0,
                    $int::BITS % 8,
                    "non-byte aligned bit width for type {}",
                    stringify!($int)
                );
                Some($int::BITS / 8)
            }
        }

        impl SerializeLexicographic for $int {
            fn serialize_lex(&self) -> Vec<u8> {
                (&self).serialize_lex()
            }
        }

        impl LexStats for $int {
            fn min() -> Vec<u8> {
                <&$int as LexStats>::min()
            }

            fn max() -> Option<Vec<u8>> {
                <&$int as LexStats>::max()
            }

            fn fixed_width() -> Option<u32> {
                <&$int as LexStats>::fixed_width()
            }
        }

        impl DeserializeLexicographic for $int {
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                let mut buf = [0u8; size_of::<$int>()];
                buf.copy_from_slice(bytes);
                Ok(Self::from_be_bytes(buf))
            }
        }
    };
}

lex_uint!(u8);
lex_uint!(u16);
lex_uint!(u32);
lex_uint!(u64);
lex_uint!(u128);

pub struct AssertSize<const N: usize>;

impl<const N: usize> AssertSize<N> {
    pub const OK: () = assert!(
        N <= u32::MAX as usize,
        "SerializeLexicographic: Fixed size byte vectors must have length <= u32::MAX"
    );
}

impl<const N: usize> SerializeLexicographic for [u8; N] {
    fn serialize_lex(&self) -> Vec<u8> {
        // compile time assertion that N fits in a u32
        let _ = AssertSize::<N>::OK;
        self.to_vec()
    }
}
impl<const N: usize> LexStats for [u8; N] {
    fn min() -> Vec<u8> {
        vec![0u8; N]
    }

    fn max() -> Option<Vec<u8>> {
        Some(vec![u8::MAX; N])
    }

    fn fixed_width() -> Option<u32> {
        Some(N as u32)
    }
}

impl<const N: usize> DeserializeLexicographic for [u8; N] {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut buf = [0u8; N];
        buf.copy_from_slice(bytes);
        Ok(buf)
    }
}

impl<T: SerializeLexicographic> SerializeLexicographic for PhantomData<T> {
    fn serialize_lex(&self) -> Vec<u8> {
        unreachable!()
    }
}

impl<T: LexStats> LexStats for PhantomData<T> {
    fn min() -> Vec<u8> {
        T::min()
    }
    fn max() -> Option<Vec<u8>> {
        T::max()
    }
    fn fixed_width() -> Option<u32> {
        T::fixed_width()
    }
}

/// Unit type for compiler support. Should never be used at runtime.
impl SerializeLexicographic for () {
    fn serialize_lex(&self) -> Vec<u8> {
        unreachable!()
    }
}

impl LexStats for () {
    fn min() -> Vec<u8> {
        vec![]
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use rand::distr::Distribution;
    use rand::distr::StandardUniform;

    use crate::*;

    #[test]
    fn should_sort_strings() {
        // test variable length strings
        for _ in 0..100 {
            let s0 = rand_utf8(rand::random::<u16>().into());
            let s1 = rand_utf8(rand::random::<u16>().into());
            let s0_bytes = s0.serialize_lex();
            let s1_bytes = s1.serialize_lex();
            assert_eq!(s0_bytes.cmp(&s1_bytes), s0.cmp(&s1));
        }
        // test equal length strings
        for _ in 0..100 {
            let len: usize = rand::random::<u16>().into();
            let s0 = rand_utf8(len);
            let s1 = rand_utf8(len);
            let s0_bytes = s0.serialize_lex();
            let s1_bytes = s1.serialize_lex();
            assert_eq!(s0_bytes.cmp(&s1_bytes), s0.cmp(&s1));
        }
    }

    #[test]
    fn should_sort_integers() {
        sample_and_sort::<u8>();
        sample_and_sort::<u16>();
        sample_and_sort::<u32>();
        sample_and_sort::<u64>();
        sample_and_sort::<u128>();
        fn sample_and_sort<
            T: SerializeLexicographic + DeserializeLexicographic + Ord + Display + std::fmt::Debug,
        >()
        where
            StandardUniform: Distribution<T>,
        {
            let mut byte_tree = BTreeMap::<Vec<u8>, usize>::default();
            let mut val_tree = BTreeMap::<T, usize>::default();
            for i in 0..1000 {
                let v = rand::random::<T>();
                byte_tree.insert(v.serialize_lex(), i);
                val_tree.insert(v, i);
            }
            for ((bytes, b_i), (v, v_i)) in byte_tree.into_iter().zip(val_tree.into_iter()) {
                assert_eq!(
                    b_i,
                    v_i,
                    "index mismatch for type {}. value {}, bytes {:?}, indices {}, {}",
                    stringify!(T),
                    v,
                    bytes,
                    v_i,
                    b_i
                );
                let (d, rem) = T::deserialize(&bytes, None).unwrap();
                assert_eq!(v, d);
                assert!(rem.is_empty());
            }
        }
    }
}
