//! Fetching a plugin's files into its staging directory with bounded
//! concurrency, each verified against the digest the manifest signed.
//!
//! A file the installed tree already holds at the manifest's digest is
//! copied into staging instead of downloaded: the digest is the manifest's
//! own, so the copy is verified exactly as a download would be, and an
//! unchanged plugin re-syncs without a single file request.
//!
//! Each per-file future owns its inputs (a cloned [`GatewayClient`], the
//! bearer, the file entry) rather than borrowing them: a borrow held across
//! the buffered await trips rustc's higher-ranked `Send` check once the sync
//! future is spawned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;

use super::ApplyError;
use super::safe_path::join_under;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{PluginEntry, PluginFile};
use crate::hash::sha256_hex;
use crate::ids::{BearerToken, Sha256Digest};

const PLUGIN_FILE_FETCH_CONCURRENCY: usize = 8;

pub(super) async fn fetch_plugin_into_staging(
    client: &GatewayClient,
    bearer: &BearerToken,
    plugin: &PluginEntry,
    stage: &Path,
    installed: &Path,
) -> Result<(), ApplyError> {
    fs::create_dir_all(stage).map_err(|e| ApplyError::Io {
        context: format!("create stage {}", stage.display()),
        source: e,
    })?;
    let mut outputs = Vec::with_capacity(plugin.files.len());
    for file in &plugin.files {
        let out = join_under(stage, &file.path)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
                context: format!("create parent {}", parent.display()),
                source: e,
            })?;
        }
        outputs.push(out);
    }

    // Why: the stream must own its futures before the first await; an
    // iterator still borrowing `plugin.files` is a borrow held across the
    // buffered await, which fails the spawned sync task's `Send` check.
    let fetches: Vec<_> = plugin
        .files
        .iter()
        .zip(outputs)
        .map(|(file, out)| {
            let reuse = installed_copy(installed, file);
            fetch_one_file(
                client.clone(),
                bearer.clone(),
                plugin.id.to_string(),
                file.clone(),
                out,
                reuse,
            )
        })
        .collect();
    let mut fetches =
        futures_util::stream::iter(fetches).buffer_unordered(PLUGIN_FILE_FETCH_CONCURRENCY);
    while let Some(fetched) = fetches.next().await {
        fetched?;
    }
    Ok(())
}

// Why: the installed file is read and hashed before the fetch future is
// built, so the reuse decision never races the promote step that replaces
// the installed tree.
fn installed_copy(installed: &Path, file: &PluginFile) -> Option<Vec<u8>> {
    let path = join_under(installed, &file.path).ok()?;
    let bytes = fs::read(path).ok()?;
    sha256_matches(&sha256_hex(&bytes), &file.sha256).then_some(bytes)
}

async fn fetch_one_file(
    client: GatewayClient,
    bearer: BearerToken,
    plugin_id: String,
    file: PluginFile,
    out: PathBuf,
    reuse: Option<Vec<u8>>,
) -> Result<(), ApplyError> {
    if let Some(bytes) = reuse {
        tracing::debug!(
            target: "bridge::sync::fetch",
            plugin_id = %plugin_id,
            path = %file.path,
            "installed file matches the manifest digest; reused without download"
        );
        return fs::write(&out, &bytes).map_err(|e| ApplyError::Io {
            context: format!("write {}", out.display()),
            source: e,
        });
    }
    let bytes = client
        .fetch_plugin_file(&bearer, &plugin_id, &file.path)
        .await?;
    let actual = sha256_hex(&bytes);
    if !sha256_matches(&actual, &file.sha256) {
        return Err(ApplyError::HashMismatch {
            what: format!("file {plugin_id}/{}", file.path),
            expected: file.sha256.clone(),
            actual,
        });
    }
    fs::write(&out, &bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", out.display()),
        source: e,
    })
}

fn sha256_matches(actual: &str, expected: &Sha256Digest) -> bool {
    actual == expected.as_str()
}
