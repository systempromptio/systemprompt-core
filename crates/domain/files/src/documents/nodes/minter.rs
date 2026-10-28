//! Minting `<writer>.<counter>` node ids.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sha2::{Digest, Sha256};
use systemprompt_identifiers::DocumentNodeId;

const WRITER_BYTES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdMinter {
    writer: String,
    last: u64,
}

impl NodeIdMinter {
    #[must_use]
    pub const fn new(writer: String, last: u64) -> Self {
        Self { writer, last }
    }

    #[must_use]
    pub fn writer(&self) -> &str {
        &self.writer
    }

    pub fn mint(&mut self) -> DocumentNodeId {
        self.last += 1;
        DocumentNodeId::new(format!("{}.{}", self.writer, self.last))
    }
}

#[must_use]
pub fn writer_tag(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let mut value = digest[..WRITER_BYTES]
        .iter()
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    let mut tag = Vec::new();
    loop {
        let digit = u8::try_from(value % 36).unwrap_or(0);
        tag.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        });
        value /= 36;
        if value == 0 {
            break;
        }
    }
    tag.reverse();
    String::from_utf8(tag).unwrap_or_default()
}
