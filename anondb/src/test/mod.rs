mod insert;
mod misc;
mod primary_key;
mod unique_index;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Document)]
pub struct TestDocument {
    pub id0: u128,
    pub id1: u128,
    pub id2: u128,
    pub id3: u128,
    pub id4: u128,
    pub str: String,
    pub bytes: Vec<u8>,
    pub bytes_fixed: [u8; 32],
}

impl Default for TestDocument {
    fn default() -> Self {
        Self {
            id0: rand::random(),
            id1: rand::random(),
            id2: rand::random(),
            id3: rand::random(),
            id4: rand::random(),
            str: rand_utf8(rand::random::<u16>().into()),
            bytes: vec![0u8; 100],
            bytes_fixed: rand::random(),
        }
    }
}
