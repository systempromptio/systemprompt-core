//! Streaming artifact download with mandatory digest verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt as _;
use sha2::{Digest as _, Sha256};

use crate::gateway::GatewayClient;
use crate::gateway::types::ReleaseManifest;
use crate::update::error::UpdateError;

// Why: a binary is tens of megabytes and the shared gateway client caps
// requests at 30s, which a slow link exceeds long before the transfer stalls.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(15);

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub received: u64,
    /// From the manifest, so progress is reportable before the first byte and
    /// does not depend on the server sending Content-Length.
    pub total: u64,
}

impl DownloadProgress {
    #[must_use]
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "progress display; a byte count large enough to lose precision here is not reachable"
        )]
        let f = self.received as f64 / self.total as f64;
        f.clamp(0.0, 1.0)
    }
}

/// Streams the platform artifact to the staging directory, hashing as it goes,
/// and returns its path.
///
/// The digest is checked before this returns and a mismatch deletes the file —
/// nothing downstream ever sees unverified bytes, because everything downstream
/// executes them.
pub async fn download_verified(
    client: &GatewayClient,
    bearer: &str,
    platform: &str,
    manifest: &ReleaseManifest,
    on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<PathBuf, UpdateError> {
    let staging = crate::config::paths::bridge_staging_dir().ok_or(UpdateError::NoStagingDir)?;
    std::fs::create_dir_all(&staging).map_err(|e| UpdateError::io(&staging, e))?;

    let dest = staging.join(format!("update-{}-{}", manifest.version, platform));
    let tmp = crate::fsutil::temp_path_for(&dest);

    let url = client.url(&format!("/v1/bridge/download/{platform}"));
    let resp = client
        .http()
        .get(&url)
        .bearer_auth(bearer)
        .timeout(DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|e| UpdateError::Download(Box::new(e)))?;
    if !resp.status().is_success() {
        return Err(UpdateError::DownloadStatus {
            status: resp.status(),
        });
    }

    let digest = stream_to_file(resp, &tmp, manifest.size, on_progress).await?;
    let actual = hex_lower(&digest);
    if !actual.eq_ignore_ascii_case(&manifest.sha256) {
        if let Err(e) = std::fs::remove_file(&tmp) {
            tracing::warn!(error = %e, path = %tmp.display(), "update: could not remove mismatched download");
        }
        return Err(UpdateError::ChecksumMismatch {
            expected: manifest.sha256.clone(),
            actual,
        });
    }

    std::fs::rename(&tmp, &dest).map_err(|e| UpdateError::io(&dest, e))?;
    tracing::info!(
        version = %manifest.version,
        path = %dest.display(),
        "update: artifact downloaded and verified"
    );
    Ok(dest)
}

async fn stream_to_file(
    resp: reqwest::Response,
    tmp: &std::path::Path,
    total: u64,
    on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<[u8; 32], UpdateError> {
    let mut file = std::fs::File::create(tmp).map_err(|e| UpdateError::io(tmp, e))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| UpdateError::Download(Box::new(e)))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| UpdateError::io(tmp, e))?;
        received = received.saturating_add(chunk.len() as u64);
        on_progress(DownloadProgress { received, total });
    }
    file.flush().map_err(|e| UpdateError::io(tmp, e))?;
    file.sync_all().map_err(|e| UpdateError::io(tmp, e))?;

    Ok(hasher.finalize().into())
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut acc, b| {
        _ = write!(acc, "{b:02x}");
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_and_zero_padded() {
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    #[test]
    fn fraction_is_clamped_and_safe_at_zero_total() {
        assert!(
            (DownloadProgress {
                received: 5,
                total: 0
            }
            .fraction()
                - 0.0)
                .abs()
                < f64::EPSILON
        );
        assert!(
            (DownloadProgress {
                received: 99,
                total: 10
            }
            .fraction()
                - 1.0)
                .abs()
                < f64::EPSILON
        );
    }
}
