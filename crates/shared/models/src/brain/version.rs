//! The pipeline version: a digest of the declared config, the brain code
//! version and every model prompt version, so any change is a new version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use systemprompt_identifiers::{FactKindId, PipelineVersion};

pub const BRAIN_CODE_VERSION: u32 = 1;

const VERSION_HEX_LEN: usize = 16;

/// Everything a pipeline version is derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineVersionInputs {
    pub declared_hash: String,
    pub code_version: u32,
    pub prompt_versions: BTreeMap<FactKindId, String>,
}

impl PipelineVersionInputs {
    #[must_use]
    pub fn new(declared_hash: impl Into<String>) -> Self {
        Self {
            declared_hash: declared_hash.into(),
            code_version: BRAIN_CODE_VERSION,
            prompt_versions: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_prompt_version(mut self, kind: FactKindId, version: impl Into<String>) -> Self {
        self.prompt_versions.insert(kind, version.into());
        self
    }

    #[must_use]
    pub fn derive(&self) -> PipelineVersion {
        let mut hasher = Sha256::new();
        hasher.update(self.declared_hash.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.code_version.to_be_bytes());
        for (kind, version) in &self.prompt_versions {
            hasher.update([0u8]);
            hasher.update(kind.as_str().as_bytes());
            hasher.update([b'=']);
            hasher.update(version.as_bytes());
        }
        let mut digest = hex::encode(hasher.finalize());
        digest.truncate(VERSION_HEX_LEN);
        PipelineVersion::new(digest)
    }
}
