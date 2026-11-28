//! A plugin of the manifest and the bundle files the instance serves for
//! it.
//!
//! The manifest entry is derived from the files: each file's digest and
//! size, and one digest over the whole bundle. A bundle is rooted on its
//! plugin manifest, so one is served for a fixture that brings none. A
//! published fixture is also a managed publication: its entry names the
//! publication, and the instance serves its installation plan, laid out as
//! core's marketplace lays one out (each file under its
//! `.systemprompt-source/<revision>/` copy and at its own path, and one
//! `SKILL.md`).

use std::collections::BTreeMap;

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{ManagedResourceId, PluginId, PublicationId, ResourceRevisionId};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::{ManagedPublicationRef, PluginEntry, PluginFile};
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};
use systemprompt_models::documents::content_hash;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, FileReadback, InstallationPlanFile, ReadbackStatus,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};
use systemprompt_models::plugin::PluginHooksRef;

const PLUGIN_VERSION: &str = "1.0.0";
const GENERATION: i64 = 1;
const SKILL_FILE: &str = "SKILL.md";
const SOURCE_DIR: &str = ".systemprompt-source";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFixture {
    pub id: PluginId,
    pub version: String,
    pub governance: bool,
    pub published: bool,
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FixtureError {
    #[error("a fixture does not encode as JSON")]
    Encode(#[from] serde_json::Error),

    #[error("a bundle digest is not a SHA-256 digest")]
    Digest(#[from] IdValidationError),

    #[error("a bundle digest is not a content digest")]
    PlanDigest,
}

impl PluginFixture {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: PluginId::new(id),
            version: PLUGIN_VERSION.to_owned(),
            governance: false,
            published: false,
            files: BTreeMap::new(),
        }
    }

    pub fn governance(id: impl Into<String>) -> Self {
        let mut plugin = Self::new(id);
        plugin.governance = true;
        plugin
    }

    /// Makes the plugin a managed publication, so the manifest names it and
    /// the instance serves its installation plan.
    pub const fn published(mut self) -> Self {
        self.published = true;
        self
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
        let sha256 = bundle_digest(&served)?;
        let mut files = Vec::with_capacity(served.len());
        for (path, bytes) in &served {
            files.push(PluginFile {
                path: path.clone(),
                sha256: digest(bytes)?,
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            });
        }
        let publication = self.published.then(|| ManagedPublicationRef {
            publication_id: self.publication_id(),
            resource_id: self.resource_id(),
            revision_id: self.revision_id(),
            generation: GENERATION,
            bundle_digest: sha256.clone(),
        });
        Ok(PluginEntry {
            id: self.id.clone(),
            version: self.version.clone(),
            sha256,
            files,
            hooks: PluginHooksRef {
                governance: self.governance,
                ..PluginHooksRef::default()
            },
            publication,
        })
    }

    pub(crate) fn publication_id(&self) -> PublicationId {
        PublicationId::new(format!("{}-publication", self.id))
    }

    pub(crate) fn resource_id(&self) -> ManagedResourceId {
        ManagedResourceId::new(format!("{}-resource", self.id))
    }

    fn revision_id(&self) -> ResourceRevisionId {
        ResourceRevisionId::new(format!("{}-revision", self.id))
    }

    /// The installation plan of a published fixture for the harness.
    pub(crate) fn plan(&self) -> Result<ConsumerInstallationPlan, FixtureError> {
        let served = self.served()?;
        let revision = self.revision_id();
        let canonical_files = served
            .iter()
            .map(|(path, bytes)| FileReadback {
                revision_id: revision.clone(),
                path: path.clone(),
                digest: ContentDigest::of(bytes),
                bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                executable: false,
                content_check: ReadbackStatus::Unavailable,
                mode_check: ReadbackStatus::Unavailable,
            })
            .collect();
        let mut runtime_files = Vec::with_capacity(served.len() * 2 + 1);
        for (path, bytes) in &served {
            runtime_files.push(plan_file(format!("{SOURCE_DIR}/{revision}/{path}"), bytes));
            if path != SKILL_FILE {
                runtime_files.push(plan_file(path.clone(), bytes));
            }
        }
        let skill = served.get(SKILL_FILE).cloned().unwrap_or_else(|| {
            format!("---\nname: \"{}\"\ndescription: \"\"\n---\n\n", self.id).into_bytes()
        });
        runtime_files.push(plan_file(SKILL_FILE.to_owned(), &skill));
        runtime_files.sort_by(|a, b| a.path.cmp(&b.path));
        let bundle = bundle_digest(&served)?;
        Ok(ConsumerInstallationPlan {
            publication_id: self.publication_id(),
            resource_id: self.resource_id(),
            revision_id: revision,
            generation: GENERATION,
            bundle_digest: ContentDigest::try_from(bundle.as_str().to_owned())
                .map_err(|_invalid| FixtureError::PlanDigest)?,
            host: EvaluatorClient::Harness,
            canonical_files,
            runtime_files,
        })
    }
}

fn plan_file(path: String, bytes: &[u8]) -> InstallationPlanFile {
    InstallationPlanFile {
        path,
        bytes: bytes.to_vec(),
        executable: false,
    }
}

fn bundle_digest(served: &BTreeMap<String, Vec<u8>>) -> Result<Sha256Digest, IdValidationError> {
    let mut whole = Vec::new();
    for (path, bytes) in served {
        whole.extend_from_slice(path.as_bytes());
        whole.extend_from_slice(digest(bytes)?.as_str().as_bytes());
    }
    digest(&whole)
}

fn digest(bytes: &[u8]) -> Result<Sha256Digest, IdValidationError> {
    Sha256Digest::try_new(content_hash(bytes).as_str())
}
