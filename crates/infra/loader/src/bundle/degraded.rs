//! The on-disk record that a boot served a fallback services tree.
//!
//! A refresh that falls back to the last-good or the bundled tree reports it
//! in the process that fell back, which is gone by the time an operator runs
//! `cloud doctor`. The record outlives the process: it is written when a
//! fallback is served and removed by the next composition that loads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;

use serde::{Deserialize, Serialize};

use super::cache::BundleCache;
use super::error::{BundleError, BundleResult};
use crate::services_root::ServicesProvenance;

const DEGRADED_FILE: &str = "degraded.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedRecord {
    pub serving: String,
    pub error: String,
    pub recorded_at: String,
}

impl DegradedRecord {
    #[must_use]
    pub fn for_provenance(provenance: &ServicesProvenance) -> Option<Self> {
        let (serving, error) = match provenance {
            ServicesProvenance::LastGood { error, .. } => ("last_good", error),
            ServicesProvenance::BundledFallback { error } => ("bundled", error),
            ServicesProvenance::Bundled | ServicesProvenance::Fetched { .. } => return None,
        };
        Some(Self {
            serving: serving.to_owned(),
            error: error.clone(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}

impl BundleCache {
    #[must_use]
    pub fn read_degraded(&self) -> Option<DegradedRecord> {
        let raw = fs::read_to_string(self.root().join(DEGRADED_FILE)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn write_degraded(&self, record: &DegradedRecord) -> BundleResult<()> {
        self.prepare()?;
        let body = serde_json::to_vec_pretty(record)
            .map_err(|e| BundleError::policy_context("degraded record is not serialisable", e))?;
        let tmp = self
            .root()
            .join(format!("{DEGRADED_FILE}.tmp-{}", std::process::id()));
        fs::write(&tmp, body)?;
        fs::rename(&tmp, self.root().join(DEGRADED_FILE))?;
        Ok(())
    }

    pub fn clear_degraded(&self) -> BundleResult<()> {
        match fs::remove_file(self.root().join(DEGRADED_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
