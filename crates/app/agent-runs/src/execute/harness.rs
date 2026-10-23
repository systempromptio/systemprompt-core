//! Installs a run's harness revision into the host before spawn.
//!
//! [`HarnessSource`] loads the verified revision closure for the run's
//! owner; [`materialise`] checks that the closure's root is the requested
//! revision and that the closure's digest is the requested
//! [`HarnessRef::bundle_digest`], then writes every file of every revision
//! in the closure under the host's [`HarnessTargets`], keeping the
//! executable bit. Two revisions that write one path, a path that escapes
//! the target, or any digest mismatch refuse the whole harness; nothing is
//! written past the first refusal. [`install`] runs it on a blocking thread,
//! off the async executor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use systemprompt_identifiers::{ResourceRevisionId, UserId};
use systemprompt_marketplace::managed::{ManagedError, ManagedRepository};
use systemprompt_models::managed::{AssetDigest, RevisionBundle, validate_path};

use crate::host::HarnessTargets;
use crate::layout::RunLayout;
use crate::request::{HarnessRef, RunRequest};

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("no harness source is configured for this runner")]
    NoSource,
    #[error("harness revision could not be loaded")]
    Load(#[source] ManagedError),
    #[error("harness bundle failed verification")]
    Verify(#[source] systemprompt_models::managed::RevisionBundleError),
    #[error("harness root is {found}, requested {requested}")]
    Root {
        requested: ResourceRevisionId,
        found: ResourceRevisionId,
    },
    #[error("harness digest is {}, requested {}", found.as_str(), requested.as_str())]
    Digest {
        requested: AssetDigest,
        found: AssetDigest,
    },
    #[error("harness path {path} is not a relative path inside the target")]
    Path {
        path: String,
        #[source]
        source: systemprompt_models::managed::RevisionBundleError,
    },
    #[error("harness path {0} has no bytes in the bundle")]
    MissingAsset(String),
    #[error("harness path {0} is written by two revisions")]
    Collision(String),
    #[error("harness installation task did not finish")]
    Join(#[source] tokio::task::JoinError),
    #[error("harness file {path} could not be written")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Loads a harness revision closure on behalf of a run's owner.
///
/// Held as `Arc<dyn HarnessSource>` by the run handler, hence
/// `#[async_trait]`.
#[async_trait]
pub trait HarnessSource: Send + Sync {
    async fn revision_bundle(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<RevisionBundle, HarnessError>;
}

#[async_trait]
impl HarnessSource for ManagedRepository {
    async fn revision_bundle(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<RevisionBundle, HarnessError> {
        self.get_revision_bundle(owner, revision)
            .await
            .map_err(HarnessError::Load)
    }
}

#[must_use]
pub fn target_root(targets: HarnessTargets, layout: &RunLayout, request: &RunRequest) -> PathBuf {
    match targets {
        HarnessTargets::Home(relative) => layout.home.join(relative),
        HarnessTargets::Workspace => request.workspace.clone(),
    }
}

pub async fn install(
    harness: HarnessRef,
    bundle: RevisionBundle,
    root: PathBuf,
) -> Result<Vec<PathBuf>, HarnessError> {
    tokio::task::spawn_blocking(move || materialise(&harness, &bundle, &root))
        .await
        .map_err(HarnessError::Join)?
}

pub fn materialise(
    harness: &HarnessRef,
    bundle: &RevisionBundle,
    root: &Path,
) -> Result<Vec<PathBuf>, HarnessError> {
    bundle.verify().map_err(HarnessError::Verify)?;
    if bundle.root != harness.revision_id {
        return Err(HarnessError::Root {
            requested: harness.revision_id.clone(),
            found: bundle.root.clone(),
        });
    }
    let found = bundle.digest().map_err(HarnessError::Verify)?;
    if found != harness.bundle_digest {
        return Err(HarnessError::Digest {
            requested: harness.bundle_digest.clone(),
            found,
        });
    }
    let mut plan: BTreeMap<String, (&[u8], bool)> = BTreeMap::new();
    for manifest in bundle.revisions.values() {
        for (path, entry) in &manifest.files {
            validate_path(path).map_err(|source| HarnessError::Path {
                path: path.clone(),
                source,
            })?;
            let bytes = bundle
                .assets
                .get(&entry.digest)
                .ok_or_else(|| HarnessError::MissingAsset(path.clone()))?;
            if plan
                .insert(path.clone(), (bytes.as_slice(), entry.executable))
                .is_some()
            {
                return Err(HarnessError::Collision(path.clone()));
            }
        }
    }
    let mut written = Vec::with_capacity(plan.len());
    for (path, (bytes, executable)) in plan {
        let target = root.join(&path);
        write(&target, bytes, executable)?;
        written.push(target);
    }
    Ok(written)
}

fn write(target: &Path, bytes: &[u8], executable: bool) -> Result<(), HarnessError> {
    let fail = |source| HarnessError::Write {
        path: target.to_path_buf(),
        source,
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    std::fs::write(target, bytes).map_err(fail)?;
    set_mode(target, executable).map_err(fail)
}

#[cfg(unix)]
fn set_mode(target: &Path, executable: bool) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
const fn set_mode(_target: &Path, _executable: bool) -> std::io::Result<()> {
    Ok(())
}
