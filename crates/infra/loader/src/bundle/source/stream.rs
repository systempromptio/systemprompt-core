//! Capped streaming of an HTTP body to a file, hashing as it goes.
//!
//! The cap is enforced on bytes actually received rather than on a
//! `Content-Length` header, so a remote that under-declares its length still
//! cannot exhaust the disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::bundle::error::{BundleError, BundleResult};

pub(super) async fn stream_to_file(
    response: reqwest::Response,
    into: &Path,
    source_name: &str,
    max_bytes: u64,
) -> BundleResult<String> {
    if let Some(parent) = into.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(into).await?;
    let mut hasher = Sha256::new();
    let mut budget = max_bytes;
    let mut response = response;

    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|e| BundleError::fetch(source_name, e))?;
        let Some(chunk) = chunk else { break };
        budget = budget
            .checked_sub(chunk.len() as u64)
            .ok_or(BundleError::TooLarge { bytes: max_bytes })?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }

    file.flush().await?;
    file.sync_all().await?;
    Ok(hex::encode(hasher.finalize()))
}
