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
    pub bytes: Vec<u8>,
}

impl LexicographicKey {
    /// Append a slice representing a lexicographically sortable key.
    pub fn append_key_slice(&mut self, slice: &[u8]) {
        if !self.bytes.is_empty() {
            self.append_separator();
        }
        self.bytes.extend_from_slice(slice);
    }

    /// Append a 0x01 byte that will sort all longer keys before this key.
    pub fn append_upper_inclusive_byte(&mut self) {
        self.bytes.push(0x01);
    }

    pub fn append_separator(&mut self) {
        self.bytes.push(0x00);
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}
