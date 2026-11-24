//! The on-disk record of sources held back on their previous bundle.
//!
//! Under `on_fetch_failure: use_last_good` a source that cannot be reached, or
//! whose newest bundle fails verification, keeps the bundle already cached for
//! it while every other source advances. The composition that results loads
//! and is served, so nothing about the active root says a source was held
//! back; this record does. Every composition that loads rewrites it with
//! exactly the sources it held back, so an absent record means each source
//! serves what it resolves to. A record that cannot be read says nothing, so
//! reading one is an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::cache::BundleCache;
use super::error::{BundleError, BundleResult, VerifyFailure};

const REFUSALS_FILE: &str = "refusals.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    Fetch,
    Auth,
    RequiresCore,
    Verify,
    Other,
}

impl RefusalKind {
    #[must_use]
    pub const fn of(error: &BundleError) -> Self {
        match error {
            BundleError::Fetch { .. } => Self::Fetch,
            BundleError::Auth { .. } => Self::Auth,
            BundleError::Verify(VerifyFailure::RequiresCore { .. }) => Self::RequiresCore,
            BundleError::Verify(_) => Self::Verify,
            BundleError::Extract { .. }
            | BundleError::Io(_)
            | BundleError::Policy { .. }
            | BundleError::NoSharedComposition { .. }
            | BundleError::Ownership { .. }
            | BundleError::FileCollision { .. }
            | BundleError::SourceMissing { .. }
            | BundleError::TooLarge { .. } => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRefusal {
    pub source: String,
    pub kind: RefusalKind,
    pub reason: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempted_digest: Option<String>,

    pub serving_version: String,
    pub serving_digest: String,
    pub recorded_at: DateTime<Utc>,
}

impl BundleCache {
    pub fn read_refusals(&self) -> BundleResult<Vec<SourceRefusal>> {
        let raw = match fs::read_to_string(self.root().join(REFUSALS_FILE)) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        serde_json::from_str(&raw)
            .map_err(|e| BundleError::policy_context("refusal record does not parse", e))
    }

    pub fn write_refusals(&self, refusals: &[SourceRefusal]) -> BundleResult<()> {
        let path = self.root().join(REFUSALS_FILE);
        if refusals.is_empty() {
            return match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            };
        }
        self.prepare()?;
        let body = serde_json::to_vec_pretty(refusals)
            .map_err(|e| BundleError::policy_context("refusal record is not serialisable", e))?;
        let tmp = self
            .root()
            .join(format!("{REFUSALS_FILE}.tmp-{}", std::process::id()));
        fs::write(&tmp, body)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}
