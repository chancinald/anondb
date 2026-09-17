use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq)]
pub struct LexStats {
    pub min: Vec<u8>,
    pub max: Option<Vec<u8>>,
    pub fixed_width: Option<u32>,
}

/// Allow a type to be serialized to lexicographically comparable bytes
pub trait SerializeLexicographic {
    fn serialize_lex(&self) -> Vec<u8>;
    fn min() -> Vec<u8>;
    fn max() -> Option<Vec<u8>> {
        None
    }
    fn fixed_width() -> Option<u32> {
        None
    }
    fn stats(&self) -> LexStats {
        LexStats {
            min: Self::min(),
            max: Self::max(),
            fixed_width: Self::fixed_width(),
        }
    }
}

impl<T: SerializeLexicographic> SerializeLexicographic for Option<T> {
    fn serialize_lex(&self) -> Vec<u8> {
        match self {
            Some(v) => vec![vec![0x01], SerializeLexicographic::serialize_lex(v)].concat(),
            None => vec![0x00],
        }
    }

    fn min() -> Vec<u8> {
        vec![0x00]
    }

    fn max() -> Option<Vec<u8>> {
        T::max().map(|v| vec![vec![0x01], v].concat())
    }
}

impl SerializeLexicographic for bool {
    fn serialize_lex(&self) -> Vec<u8> {
        if *self { vec![0x01] } else { vec![0x00] }
    }

    fn min() -> Vec<u8> {
        vec![0x00]
    }

    fn max() -> Option<Vec<u8>> {
        Some(vec![0x01])
    }
}

mod sealed {
    use super::SerializeLexicographic;

    trait ToBytes {
        fn to_bytes(self) -> Vec<u8>;
    }

    impl ToBytes for String {
        fn to_bytes(self) -> Vec<u8> {
            self.into_bytes()
        }
    }

    impl ToBytes for &String {
        fn to_bytes(self) -> Vec<u8> {
            self.as_bytes().to_vec()
        }
    }

    impl ToBytes for &str {
        fn to_bytes(self) -> Vec<u8> {
            self.as_bytes().to_vec()
        }
    }

    impl ToBytes for Vec<u8> {
        fn to_bytes(self) -> Vec<u8> {
            self
        }
    }

    impl<T: ToBytes + Clone> SerializeLexicographic for T {
        fn serialize_lex(&self) -> Vec<u8> {
            vec![self.clone().to_bytes(), vec![0x00]].concat()
        }

        fn min() -> Vec<u8> {
            vec![0x00]
        }
    }
}

macro_rules! lex_uint {
    ($int:ident) => {
        impl SerializeLexicographic for &$int {
            fn serialize_lex(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }

            fn min() -> Vec<u8> {
                $int::MIN.to_be_bytes().to_vec()
            }

            fn max() -> Option<Vec<u8>> {
                Some($int::MAX.to_be_bytes().to_vec())
            }

            fn fixed_width() -> Option<u32> {
                Some($int::BITS / 8)
            }
        }

        impl SerializeLexicographic for $int {
            fn serialize_lex(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }

            fn min() -> Vec<u8> {
                $int::MIN.to_be_bytes().to_vec()
            }

            fn max() -> Option<Vec<u8>> {
                Some($int::MAX.to_be_bytes().to_vec())
            }

            fn fixed_width() -> Option<u32> {
                Some($int::BITS / 8)
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

impl<T: SerializeLexicographic> SerializeLexicographic for PhantomData<T> {
    fn serialize_lex(&self) -> Vec<u8> {
        unreachable!()
    }
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

    fn min() -> Vec<u8> {
        unreachable!()
    }

    fn max() -> Option<Vec<u8>> {
        unreachable!()
    }
}

#[cfg(test)]
mod test {
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
}
