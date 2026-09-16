//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackError, Result};
use crate::ids::BearerToken;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{ConsumerInstallationId, DeviceId, UserId};

/// The origin a device credential is scoped to.
///
/// Distinct from [`crate::config::trust::GatewayIdentity`] on purpose: a
/// device credential travels only over TLS (loopback excepted) and the
/// consumer API is origin-rooted, so the path is not part of the scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GatewayOrigin(String);

impl GatewayOrigin {
    pub fn parse(gateway: &str) -> Result<Self> {
        let url = url::Url::parse(gateway)?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (url.scheme() != "https" && !(url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(FeedbackError::Scope);
        }
        Ok(Self(url.origin().ascii_serialization()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GatewayOrigin {
    type Error = FeedbackError;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}

impl From<GatewayOrigin> for String {
    fn from(value: GatewayOrigin) -> Self {
        value.0
    }
}

impl std::fmt::Display for GatewayOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Enrollment {
    pub gateway: GatewayOrigin,
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
        let gateway = GatewayOrigin::parse(gateway)?;
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
        if enrollment.gateway != GatewayOrigin::parse(gateway)? {
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
