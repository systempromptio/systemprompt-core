//! Self-issued device enrolment.
//!
//! Derives a stable fingerprint from the install id and the signed-in user,
//! then has the gateway enrol it and issue the consumer credential — no
//! administrator-issued token required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::UserId;

use super::credentials::Enrollment;
use super::{FeedbackError, Result};
use crate::gateway::GatewayClient;
use crate::gateway::types::SelfEnrollRequest;
use crate::ids::BearerToken;

const FINGERPRINT_DOMAIN: &str = "systemprompt-bridge-device/v1";
const DEFAULT_LABEL: &str = "bridge";

/// What one install enrols as: the install id and signed-in user that form
/// the fingerprint, the human label the gateway records, and whether a
/// stored credential is rotated rather than reused.
#[derive(Debug, Clone)]
pub struct SelfEnrolment<'a> {
    pub install_id: &'a str,
    pub user_id: &'a UserId,
    pub label: Option<String>,
    pub force_rotate: bool,
}

// Why: `user_device_certs.fingerprint` is globally unique, so the user is
// folded in to keep one install's fingerprint distinct per signed-in user.
pub fn device_fingerprint(install_id: &str, user_id: &UserId) -> String {
    let material = format!("{FINGERPRINT_DOMAIN}\n{install_id}\n{}", user_id.as_str());
    crate::hash::sha256_hex(material.as_bytes())
}

pub async fn ensure_self_enrolled(
    client: &GatewayClient,
    bearer: &BearerToken,
    enrolment: &SelfEnrolment<'_>,
) -> Result<Enrollment> {
    let root = super::metadata_root()?;
    enroll_into(&root, client, bearer, enrolment).await
}

pub async fn enroll_into(
    root: &Path,
    client: &GatewayClient,
    bearer: &BearerToken,
    enrolment: &SelfEnrolment<'_>,
) -> Result<Enrollment> {
    let gateway = client.base_url_str();
    let previous = previous_enrolment(root, gateway)?;
    if !enrolment.force_rotate
        && let Some(existing) = previous
            .as_ref()
            .filter(|existing| existing.consumer_id == *enrolment.user_id)
    {
        return Ok(existing.clone());
    }

    let request = SelfEnrollRequest {
        fingerprint: device_fingerprint(enrolment.install_id, enrolment.user_id),
        label: enrolment
            .label
            .clone()
            .unwrap_or_else(|| DEFAULT_LABEL.to_owned()),
    };
    let response = client.enroll_device(bearer, &request).await?;

    let mut enrollment = Enrollment::new(
        gateway,
        response.device_id,
        response.consumer_id,
        BearerToken::new(response.credential),
    )?;
    if let Some(previous) = previous
        && previous.consumer_id == enrollment.consumer_id
        && previous.device_id == enrollment.device_id
    {
        enrollment.installation_id = previous.installation_id;
    }
    crate::fsutil::create_dir_all_mode_0700(root)?;
    enrollment.save(root)?;
    Ok(enrollment)
}

// Why: no stored enrolment, or one for another gateway, means "enrol afresh";
// an unreadable or corrupt store is surfaced rather than silently replaced.
pub fn previous_enrolment(root: &Path, gateway: &str) -> Result<Option<Enrollment>> {
    match Enrollment::load(root, gateway) {
        Ok(enrollment) => Ok(Some(enrollment)),
        Err(FeedbackError::EnrollmentRequired | FeedbackError::Scope) => Ok(None),
        Err(error) => Err(error),
    }
}
