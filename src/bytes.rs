use std::borrow::Borrow;

use anyhow::Result;
use redb::Key;
use redb::Value;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bytes {
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
    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
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
    type AsBytes<'a> = Vec<u8>;
    type SelfType<'a> = Bytes;

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("bytes")
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        value.bytes.clone()
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
