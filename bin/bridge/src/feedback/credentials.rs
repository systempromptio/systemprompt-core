//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackError, Result};
use crate::ids::BearerToken;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{ConsumerInstallationId, DeviceId, UserId};

#[derive(Clone, Serialize, Deserialize)]
pub struct Enrollment {
    pub gateway: String,
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub installation_id: ConsumerInstallationId,
    credential: BearerToken,
}

impl std::fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enrollment")
            .field("gateway", &self.gateway)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl Enrollment {
    pub fn new(
        gateway: &str,
        device_id: DeviceId,
        consumer_id: UserId,
        credential: BearerToken,
    ) -> Result<Self> {
        if !credential.as_str().starts_with("sp_device_") || credential.as_str().len() > 256 {
            return Err(FeedbackError::EnrollmentRequired);
        }
        let gateway = canonical_gateway(gateway)?;
        Ok(Self {
            gateway,
            device_id,
            consumer_id,
            installation_id: ConsumerInstallationId::generate(),
            credential,
        })
    }

    pub fn credential(&self) -> &str {
        self.credential.as_str()
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        crate::fsutil::atomic_write_0600(&root.join("device.json"), &serde_json::to_vec(self)?)?;
        Ok(())
    }

    pub fn load(root: &Path, gateway: &str) -> Result<Self> {
        let path = root.join("device.json");
        if !path.exists() {
            return Err(FeedbackError::EnrollmentRequired);
        }
        if std::fs::metadata(&path)?.len() > 4096 {
            return Err(FeedbackError::Scope);
        }
        let bytes = std::fs::read(&path)?;
        let enrollment: Self = serde_json::from_slice(&bytes)?;
        if enrollment.gateway != canonical_gateway(gateway)? {
            return Err(FeedbackError::Scope);
        }
        Ok(enrollment)
    }

    pub fn outbox_path(&self, root: &Path) -> PathBuf {
        let scope = format!("{}:{}:{}", self.gateway, self.consumer_id, self.device_id);
        root.join(format!(
            "{}.json",
            crate::hash::sha256_hex(scope.as_bytes())
        ))
    }
}

pub fn canonical_gateway(gateway: &str) -> Result<String> {
    let url = url::Url::parse(gateway)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(FeedbackError::Scope);
    }
    Ok(url.origin().ascii_serialization())
}
