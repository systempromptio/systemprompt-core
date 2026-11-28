//! A plugin of the manifest and the bundle files the instance serves for
//! it.
//!
//! The manifest entry is derived from the files: each file's digest and
//! size, and one digest over the whole bundle. A bundle is rooted on its
//! plugin manifest, so one is served for a fixture that brings none.

use std::collections::BTreeMap;

use systemprompt_identifiers::PluginId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::{PluginEntry, PluginFile};
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};
use systemprompt_models::documents::content_hash;
use systemprompt_models::plugin::PluginHooksRef;

const PLUGIN_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFixture {
    pub id: PluginId,
    pub version: String,
    pub governance: bool,
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FixtureError {
    #[error("a fixture does not encode as JSON")]
    Encode(#[from] serde_json::Error),

    #[error("a bundle digest is not a SHA-256 digest")]
    Digest(#[from] IdValidationError),
}

impl PluginFixture {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: PluginId::new(id),
            version: PLUGIN_VERSION.to_owned(),
            governance: false,
            files: BTreeMap::new(),
        }
    }

    pub fn governance(id: impl Into<String>) -> Self {
        let mut plugin = Self::new(id);
        plugin.governance = true;
        plugin
    }

    pub fn file(mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.files.insert(path.into(), bytes.into());
        self
    }

    pub(crate) fn served(&self) -> Result<BTreeMap<String, Vec<u8>>, serde_json::Error> {
        let mut served = self.files.clone();
        if !served.contains_key(PLUGIN_MANIFEST_RELPATH) {
            let manifest = PluginManifest {
                name: self.id.as_str().to_owned(),
                version: self.version.clone(),
                ..PluginManifest::default()
            };
            served.insert(
                PLUGIN_MANIFEST_RELPATH.to_owned(),
                serde_json::to_vec(&manifest)?,
            );
        }
        Ok(served)
    }

    pub(crate) fn entry(&self) -> Result<PluginEntry, FixtureError> {
        let served = self.served()?;
        let mut whole = Vec::new();
        let mut files = Vec::with_capacity(served.len());
        for (path, bytes) in &served {
            let sha256 = digest(bytes)?;
            whole.extend_from_slice(path.as_bytes());
            whole.extend_from_slice(sha256.as_str().as_bytes());
            files.push(PluginFile {
                path: path.clone(),
                sha256,
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            });
        }
        Ok(PluginEntry {
            id: self.id.clone(),
            version: self.version.clone(),
            sha256: digest(&whole)?,
            files,
            hooks: PluginHooksRef {
                governance: self.governance,
                ..PluginHooksRef::default()
            },
            publication: None,
        })
    }
}

fn digest(bytes: &[u8]) -> Result<Sha256Digest, IdValidationError> {
    Sha256Digest::try_new(content_hash(bytes).as_str())
}
