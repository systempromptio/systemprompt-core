//! Plugin bundle files for the projection, downloaded with the account's JWT
//! and cached by their SHA-256: in memory for the loop, and on disk under the
//! cache dir, verified on every read, so an unchanged file is never downloaded
//! again. A file the instance does not serve is left out for the projection to
//! report; a transport failure fails the poll. Bytes that do not match the
//! manifest's digest are handed on (the projection refuses them) but never
//! kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use sp_client::{ClientError, CredentialSource, with_jwt};
use sp_projection::BundleFiles;
use sp_protocol::ids::PluginId;
use sp_protocol::names::sha256_hex;
use sp_protocol::wire::manifest::Sha256Digest;
use sp_secure::{read_optional, remove_if_exists, write_private};

use crate::ports::Instance;

const BUNDLES_DIR: &str = "bundles";

/// The plugin bundle files one account's loop holds, keyed by digest, over
/// their disk cache.
#[derive(Debug)]
pub struct BundleCache {
    dir: PathBuf,
    held: HashMap<Sha256Digest, Vec<u8>>,
}

impl BundleCache {
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            dir: cache_dir.join(BUNDLES_DIR),
            held: HashMap::new(),
        }
    }

    pub(crate) async fn fetch<I: Instance>(
        &mut self,
        instance: &I,
        source: &dyn CredentialSource,
        wanted: &[(PluginId, String, Sha256Digest)],
    ) -> Result<BundleFiles, ClientError> {
        let mut files = BundleFiles::new();
        for (plugin, path, digest) in wanted {
            if let Some(bytes) = self.held.get(digest) {
                files.insert(plugin, path.clone(), bytes.clone());
                continue;
            }
            if let Some(bytes) = read_cached(self.dir.clone(), digest.clone()).await {
                self.held.insert(digest.clone(), bytes.clone());
                files.insert(plugin, path.clone(), bytes);
                continue;
            }
            let Some(bytes) = download(instance, source, plugin, path).await? else {
                continue;
            };
            if sha256_hex(&bytes) == digest.as_str() {
                self.held.insert(digest.clone(), bytes.clone());
                write_cached(self.dir.clone(), digest.clone(), bytes.clone()).await;
            } else {
                tracing::warn!(plugin = %plugin.as_str(), file = %path, "a bundle file does not match its digest");
            }
            files.insert(plugin, path.clone(), bytes);
        }
        let keep: HashSet<Sha256Digest> =
            wanted.iter().map(|(_, _, digest)| digest.clone()).collect();
        self.held.retain(|digest, _| keep.contains(digest));
        prune_cached(self.dir.clone(), keep).await;
        Ok(files)
    }
}

async fn download<I: Instance>(
    instance: &I,
    source: &dyn CredentialSource,
    plugin: &PluginId,
    path: &str,
) -> Result<Option<Vec<u8>>, ClientError> {
    let fetched = with_jwt(source, move |jwt| async move {
        instance.plugin_file(&jwt, plugin, path).await
    })
    .await;
    match fetched {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error @ (ClientError::Protocol { .. } | ClientError::InvalidInput { .. })) => {
            tracing::warn!(plugin = %plugin.as_str(), file = %path, error = %error, "the instance does not serve a bundle file");
            Ok(None)
        },
        Err(error) => Err(error),
    }
}

async fn read_cached(dir: PathBuf, digest: Sha256Digest) -> Option<Vec<u8>> {
    let read = tokio::task::spawn_blocking(move || {
        let path = dir.join(digest.as_str());
        match read_optional(&path) {
            Ok(Some(bytes)) if sha256_hex(&bytes) == digest.as_str() => Some(bytes),
            Ok(Some(_)) => {
                tracing::warn!(path = %path.display(), "a cached bundle file does not match its digest; dropping it");
                if let Err(error) = remove_if_exists(&path) {
                    tracing::debug!(path = %path.display(), error = %error, "removing a cached bundle file failed");
                }
                None
            },
            Ok(None) => None,
            Err(error) => {
                tracing::debug!(path = %path.display(), error = %error, "reading a cached bundle file failed");
                None
            },
        }
    })
    .await;
    read.unwrap_or_else(|error| {
        tracing::debug!(error = %error, "reading a cached bundle file did not finish");
        None
    })
}

async fn write_cached(dir: PathBuf, digest: Sha256Digest, bytes: Vec<u8>) {
    let written =
        tokio::task::spawn_blocking(move || write_private(&dir.join(digest.as_str()), &bytes))
            .await;
    match written {
        Ok(Ok(())) => {},
        Ok(Err(error)) => tracing::debug!(error = %error, "caching a bundle file failed"),
        Err(error) => tracing::debug!(error = %error, "caching a bundle file did not finish"),
    }
}

async fn prune_cached(dir: PathBuf, keep: HashSet<Sha256Digest>) {
    let pruned = tokio::task::spawn_blocking(move || {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let cached = name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit());
            if cached
                && !keep.iter().any(|digest| digest.as_str() == name)
                && let Err(error) = remove_if_exists(&entry.path())
            {
                tracing::debug!(error = %error, "pruning a cached bundle file failed");
            }
        }
    })
    .await;
    if let Err(error) = pruned {
        tracing::debug!(error = %error, "pruning cached bundle files did not finish");
    }
}
