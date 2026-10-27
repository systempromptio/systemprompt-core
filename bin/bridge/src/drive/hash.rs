//! Streaming SHA-256 into the [`ContentHash`] a blob is addressed by.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_identifiers::ContentHash;

use super::error::{DriveError, DriveResult};

const CHUNK: usize = 64 * 1024;

#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash::new(crate::hash::hex_encode(&Sha256::digest(bytes)))
}

pub fn hash_file(path: &Path) -> DriveResult<(ContentHash, u64)> {
    let mut file =
        std::fs::File::open(path).map_err(|e| DriveError::io("open for hashing", path, e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut size = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| DriveError::io("read for hashing", path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((
        ContentHash::new(crate::hash::hex_encode(&hasher.finalize())),
        size,
    ))
}

pub fn verify(expected: &ContentHash, bytes: &[u8]) -> DriveResult<()> {
    let actual = hash_bytes(bytes);
    if &actual == expected {
        Ok(())
    } else {
        Err(DriveError::HashMismatch {
            expected: expected.clone(),
            actual,
        })
    }
}
