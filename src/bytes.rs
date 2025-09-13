use std::borrow::Borrow;
use std::sync::Arc;

use anyhow::Result;
use redb::Key;
use redb::Value;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Bytes {
    #[serde(with = "serde_bytes")]
    bytes: Vec<u8>,
}

impl Borrow<[u8]> for Bytes {
    fn borrow(&self) -> &[u8] {
        &self.bytes
    }
}

impl Borrow<Vec<u8>> for Bytes {
    fn borrow(&self) -> &Vec<u8> {
        &self.bytes
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Into<Vec<u8>> for Bytes {
    fn into(self) -> Vec<u8> {
        self.bytes
    }
}

impl<'a> Into<&'a str> for &'a Bytes {
    fn into(self) -> &'a str {
        str::from_utf8(&self.bytes).unwrap()
    }
}

impl Into<u64> for Bytes {
    fn into(self) -> u64 {
        let mut bytes = [0u8; 8];
        assert_eq!(self.bytes.len(), 8);
        bytes.copy_from_slice(&self.bytes);
        u64::from_le_bytes(bytes)
    }
}

impl From<Arc<[u8]>> for Bytes {
    fn from(value: Arc<[u8]>) -> Self {
        Bytes {
            bytes: value.to_vec(),
        }
    }
}

impl From<String> for Bytes {
    fn from(value: String) -> Self {
        Bytes {
            bytes: value.into_bytes(),
        }
    }
}

impl From<&str> for Bytes {
    fn from(value: &str) -> Self {
        Bytes {
            bytes: value.as_bytes().to_vec(),
        }
    }
}

impl From<u64> for Bytes {
    fn from(value: u64) -> Self {
        Bytes {
            bytes: value.to_le_bytes().to_vec(),
        }
    }
}

impl From<&Vec<u8>> for Bytes {
    fn from(value: &Vec<u8>) -> Self {
        Bytes {
            bytes: value.clone(),
        }
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(value: Vec<u8>) -> Self {
        Bytes { bytes: value }
    }
}

impl From<&[u8]> for Bytes {
    fn from(value: &[u8]) -> Self {
        Bytes {
            bytes: value.to_vec(),
        }
    }
}

impl Bytes {
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn encode<T>(value: &T) -> Result<Self>
    where
        T: Serialize,
    {
        Ok(Self {
            bytes: rmp_serde::to_vec(value)?,
        })
    }

    pub fn parse<T>(&self) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        Ok(rmp_serde::from_slice(&self.bytes)?)
    }
}

impl Key for Bytes {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

impl Value for Bytes {
    type AsBytes<'a> = &'a [u8];
    type SelfType<'a> = Bytes;

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("anondb_bytes")
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        &value.bytes
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        Bytes {
            bytes: data.to_vec(),
        }
    }

    fn fixed_width() -> Option<usize> {
        None
    }
}
