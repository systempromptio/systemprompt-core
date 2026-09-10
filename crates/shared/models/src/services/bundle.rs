//! Manifest, ownership and state types for a packaged services bundle.
//!
//! A bundle is a gzipped tar holding [`BUNDLE_MANIFEST_FILE`] and a
//! `services/` subtree. The manifest is the verification surface: the archive
//! digest pins the bytes, the optional signature attests the publisher, the
//! per-file `sha256` values check extraction, and
//! [`ServicesBundleManifest::content_hash`] keys the cache and the authz
//! reconcile.
//!
//! [`FileEntry`] serialises its digest under the key `checksum`, the name the
//! services download manifest has always used on the wire.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BUNDLE_MANIFEST_FILE: &str = "bundle.json";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub const BUNDLE_MEDIA_TYPE: &str = "application/vnd.systemprompt.services-bundle.v1.tar+gzip";
pub const BUNDLE_SIGNATURE_ALG: &str = "ed25519";

pub const BUNDLE_ALLOWED_DIRS: &[&str] = &[
    "access-control",
    "agents",
    "ai",
    "artifacts",
    "config",
    "content",
    "external_agents",
    "gateway",
    "governance",
    "hooks",
    "marketplaces",
    "mcp",
    "plugins",
    "rules",
    "scheduler",
    "skills",
    "slack",
    "web",
];

pub const MARKETPLACE_BUNDLE_DIRS: &[&str] = &[
    "marketplaces",
    "plugins",
    "skills",
    "rules",
    "hooks",
    "artifacts",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,

    #[serde(rename = "checksum")]
    pub sha256: String,

    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSourceInfo {
    #[serde(default)]
    pub repo: Option<String>,

    #[serde(default)]
    pub commit: Option<String>,

    #[serde(default)]
    pub workflow_run: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleOwnership {
    #[serde(default)]
    pub marketplaces: Vec<String>,

    #[serde(default)]
    pub plugins: Vec<String>,

    #[serde(default)]
    pub skills: Vec<String>,

    #[serde(default)]
    pub rules: Vec<String>,

    #[serde(default)]
    pub hooks: Vec<String>,

    #[serde(default)]
    pub artifacts: Vec<String>,

    #[serde(default)]
    pub dirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesBundleManifest {
    pub format: u32,

    pub version: String,

    pub created_at: DateTime<Utc>,

    pub requires_core: String,

    #[serde(default)]
    pub source: BundleSourceInfo,

    #[serde(default)]
    pub files: Vec<FileEntry>,

    pub content_hash: String,

    #[serde(default)]
    pub total_size: u64,

    #[serde(default)]
    pub owns: BundleOwnership,
}

impl ServicesBundleManifest {
    #[must_use]
    pub fn compute_content_hash(files: &[FileEntry]) -> String {
        let mut lines: Vec<String> = files
            .iter()
            .map(|f| format!("{}\0{}\n", f.path, f.sha256))
            .collect();
        lines.sort();

        let mut hasher = Sha256::new();
        for line in &lines {
            hasher.update(line.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    #[must_use]
    pub fn is_marketplace_only(&self) -> bool {
        !self.owns.dirs.is_empty()
            && self
                .owns
                .dirs
                .iter()
                .all(|d| MARKETPLACE_BUNDLE_DIRS.contains(&d.as_str()))
    }

    pub fn core_satisfies(&self, core_version: &str) -> Result<bool, semver::Error> {
        let req = semver::VersionReq::parse(&self.requires_core)?;
        let version = semver::Version::parse(core_version)?;
        Ok(req.matches(&version))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSignature {
    pub alg: String,

    pub key_id: String,

    pub sig_b64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedBundleManifest {
    pub manifest: ServicesBundleManifest,

    #[serde(default)]
    pub signature: Option<BundleSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSourceState {
    pub digest: String,

    pub version: String,

    pub content_hash: String,

    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesBundleState {
    #[serde(default)]
    pub composed_hash: String,

    #[serde(default)]
    pub last_reconciled_hash: Option<String>,

    #[serde(default)]
    pub sources: BTreeMap<String, BundleSourceState>,
}
