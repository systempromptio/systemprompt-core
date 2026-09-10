//! On-disk layout for fetched and composed bundles.
//!
//! Everything under the cache is content-addressed, so a re-fetch of an
//! unchanged bundle is a no-op and a rollback is a re-point rather than a
//! download. `current` is a symlink swapped by `rename`, which is atomic on
//! the same filesystem: a reader either sees the whole previous composition
//! or the whole new one, never a half-copied tree. State is written the same
//! way — temp file, then rename.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::services::bundle::{
    BUNDLE_MANIFEST_FILE, ServicesBundleState, SignedBundleManifest,
};

use super::error::{BundleError, BundleResult};

const STATE_FILE: &str = "state.json";
const CURRENT_LINK: &str = "current";
const KEEP_PER_BUNDLE: usize = 2;
const KEEP_COMPOSED: usize = 2;

#[derive(Debug, Clone)]
pub struct BundleCache {
    root: PathBuf,
}

impl BundleCache {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn prepare(&self) -> BundleResult<()> {
        fs::create_dir_all(self.root.join("bundles"))?;
        fs::create_dir_all(self.root.join("composed"))?;
        Ok(())
    }

    #[must_use]
    pub fn bundle_dir(&self, name: &str, content_hash: &str) -> PathBuf {
        self.root.join("bundles").join(name).join(content_hash)
    }

    #[must_use]
    pub fn composed_dir(&self, composed_hash: &str) -> PathBuf {
        self.root.join("composed").join(composed_hash)
    }

    #[must_use]
    pub fn current_link(&self) -> PathBuf {
        self.root.join(CURRENT_LINK)
    }

    #[must_use]
    pub fn current_root(&self) -> Option<PathBuf> {
        let link = self.current_link();
        link.exists().then_some(link)
    }

    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }

    #[must_use]
    pub fn read_state(&self) -> ServicesBundleState {
        fs::read_to_string(self.state_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn read_manifest(
        &self,
        name: &str,
        content_hash: &str,
    ) -> BundleResult<SignedBundleManifest> {
        let path = self
            .bundle_dir(name, content_hash)
            .join(BUNDLE_MANIFEST_FILE);
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw).map_err(|e| {
            BundleError::policy(format!("cached bundle.json for {name} does not parse: {e}"))
        })
    }

    pub fn write_state(&self, state: &ServicesBundleState) -> BundleResult<()> {
        self.prepare()?;
        let body = serde_json::to_vec_pretty(state)
            .map_err(|e| BundleError::policy(format!("state is not serialisable: {e}")))?;
        let tmp = self
            .root
            .join(format!("{STATE_FILE}.tmp-{}", std::process::id()));
        fs::write(&tmp, body)?;
        fs::rename(&tmp, self.state_path())?;
        Ok(())
    }

    pub fn swap_current(&self, target: &Path) -> BundleResult<()> {
        self.prepare()?;
        let staging = self
            .root
            .join(format!("{CURRENT_LINK}.tmp-{}", std::process::id()));
        if staging.exists() || fs::symlink_metadata(&staging).is_ok() {
            fs::remove_file(&staging)?;
        }
        symlink(target, &staging)?;
        fs::rename(&staging, self.current_link())?;
        Ok(())
    }

    pub fn gc(&self, keep_composed: &str) -> BundleResult<()> {
        let bundles = self.root.join("bundles");
        if bundles.is_dir() {
            for entry in fs::read_dir(&bundles)? {
                let dir = entry?.path();
                if dir.is_dir() {
                    retain_newest(&dir, KEEP_PER_BUNDLE, "")?;
                }
            }
        }
        let composed = self.root.join("composed");
        if composed.is_dir() {
            retain_newest(&composed, KEEP_COMPOSED, keep_composed)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

fn retain_newest(dir: &Path, keep: usize, pinned: &str) -> BundleResult<()> {
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        let modified = path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        entries.push((modified, path));
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));

    for (_time, path) in entries.into_iter().skip(keep) {
        let is_pinned = path
            .file_name()
            .is_some_and(|n| !pinned.is_empty() && n == pinned);
        if is_pinned {
            continue;
        }
        if let Err(e) = fs::remove_dir_all(&path) {
            tracing::warn!(path = %path.display(), error = %e, "Failed to prune cached bundle");
        }
    }
    Ok(())
}
