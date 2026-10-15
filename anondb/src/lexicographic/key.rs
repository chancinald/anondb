use super::*;

/// A vector of bytes representing a lexicographically sortable set of keys. Each key is separated
/// by a byte 0x00 to allow partial index searches.
///
/// If I have an index (id: u8, created_at: u8, name: String ) and i want to filter by
/// { id = 0, created_at = gt(1) && lt(99) }
///
/// I need to sort by 00000000100000000..0000000063000000. But i need to include all keys that are
/// longer than the provided slice. e.g. 0000000050000000a3eb398e should be included.
///
/// To achieve this we need a separator that is a fixed value that we can use for comparison. If we
/// choose this byte as 0x00, then we can suffix the upper bound of our sort queries with
/// 0x01 to include all longer keys.
///
/// To visualize this better consider the following hex example:
///
/// We want values between 0001 and 00ff. We also want all longer hex values. We compute our
/// bounds:
///
/// lower: 0001
/// upper: 00ff01
///
/// Now the value 00ee00aabbcc sorts within this range
///
/// This strategy adds ~1 byte of overhead per field (0 bytes for indices with 1 field).
#[derive(Default, Clone)]
pub struct LexicographicKey {
    bytes: Vec<u8>,
}

impl LexicographicKey {
    /// Append a slice representing a lexicographically sortable key.
    pub fn append_key_slice(&mut self, slice: &[u8]) {
        if !self.bytes.is_empty() {
            self.append_separator();
        }
        self.bytes.extend_from_slice(slice);
    }

    pub fn append_key_vec(&mut self, mut vec: Vec<u8>) {
        if !self.bytes.is_empty() {
            self.append_separator();
        }
        self.bytes.append(&mut vec);
    }

    pub fn append(&mut self, value: &impl SerializeLexicographic) {
        self.append_key_vec(value.serialize_lex());
    }

    /// Append a 0x01 byte that will sort all longer keys before this key.
    /// Used when filtering exclusively on earlier bytes.
    ///
    /// e.g. index containing fields [A, B, C]
    /// We want to select a range of A ignoring B and C. For all A values
    /// that compare equal, the B and C values will then be compared. We can
    /// avoid this by appending a 1 byte that is compared with the separator,
    /// always sorting after, ignoring the value of B.
    pub fn append_upper_inclusive_byte(&mut self) {
        self.bytes.push(0x01);
    }

    pub fn append_separator(&mut self) {
        self.bytes.push(0x00);
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    /// Attempt to deserialize a type from the bytes
    pub fn take<T: DeserializeLexicographic>(&mut self) -> anyhow::Result<T> {
        if self.bytes.is_empty() {
            anyhow::bail!("no more bytes in key")
        }
        if let Some(width) = T::FIXED_WIDTH {
            if width as usize > self.bytes.len() {
                anyhow::bail!(
                    "not enough bytes remaining for type, need {width}, have {}",
                    self.bytes.len()
                )
            }
            let mut bytes = self.bytes.split_off(width as usize);
            std::mem::swap(&mut bytes, &mut self.bytes);
            if !self.bytes.is_empty() {
                // remove the separator byte
                if self.bytes.remove(0) != 0 {
                    anyhow::bail!("bad separator byte in key vector")
                }
            }
            T::from_bytes(&bytes)
        } else {
            T::from_bytes(&self.bytes)
        }
    }
}

impl FromIterator<Vec<u8>> for LexicographicKey {
    fn from_iter<T: IntoIterator<Item = Vec<u8>>>(iter: T) -> Self {
        let mut key = LexicographicKey::default();
        for v in iter {
            key.append_key_vec(v);
        }
        key
    }
}

impl From<Vec<u8>> for LexicographicKey {
    fn from(value: Vec<u8>) -> Self {
        Self { bytes: value }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key_sort_longer_vec_within() {
        let mut start_key = LexicographicKey::default();
        start_key.append_key_slice(&[0u8; 32]);
        let mut end_key = LexicographicKey::default();
        end_key.append_key_slice(&[u8::MAX; 32]);
        end_key.append_upper_inclusive_byte();

        let mut expected_within = LexicographicKey::default();
        expected_within.append_key_slice(&[5u8; 32]);
        expected_within.append_key_slice(&[99u8; 32]);

        let range = start_key.as_slice()..end_key.as_slice();
        assert!(range.contains(&expected_within.as_slice()));
    }
}
