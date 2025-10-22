/// Allow a type to be serialized to lexicographically comparable bytes
pub trait SerializeLexicographic {
    fn serialize_lex(&self) -> Vec<u8>;
    fn min() -> Vec<u8>;
    fn max() -> Option<Vec<u8>>;
}

impl SerializeLexicographic for String {
    fn serialize_lex(&self) -> Vec<u8> {
        vec![self.as_bytes().to_vec(), vec![0x00]].concat()
    }

    fn min() -> Vec<u8> {
        vec![0x00]
    }

    fn max() -> Option<Vec<u8>> {
        None
    }
}

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

macro_rules! lex_uint {
    ($int:ident) => {
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
        }
    };
}

lex_uint!(u8);
lex_uint!(u16);
lex_uint!(u32);
lex_uint!(u64);
lex_uint!(u128);
