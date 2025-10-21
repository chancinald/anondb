/// Allow a type to be serialized to lexicographically comparable bytes
pub trait SerializeLexicographic {
    fn serialize_lex(&self) -> Vec<u8>;
}

impl SerializeLexicographic for String {
    fn serialize_lex(&self) -> Vec<u8> {
        unimplemented!()
    }
}

impl<T: SerializeLexicographic> SerializeLexicographic for Option<T> {
    fn serialize_lex(&self) -> Vec<u8> {
        match self {
            Some(v) => SerializeLexicographic::serialize_lex(v),
            None => vec![0x00],
        }
    }
}

impl SerializeLexicographic for u8 {
    fn serialize_lex(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

impl SerializeLexicographic for u16 {
    fn serialize_lex(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

impl SerializeLexicographic for u32 {
    fn serialize_lex(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

impl SerializeLexicographic for u64 {
    fn serialize_lex(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

impl SerializeLexicographic for u128 {
    fn serialize_lex(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
