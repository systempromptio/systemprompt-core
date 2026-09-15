//! Self-issued device enrolment: derive a stable fingerprint from the
//! install id and the signed-in user, then have the gateway enrol it and
//! issue the consumer credential — no administrator-issued token required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::UserId;

use super::Result;
use super::credentials::Enrollment;
use crate::gateway::GatewayClient;
use crate::gateway::types::SelfEnrollRequest;
use crate::ids::BearerToken;
use crate::proxy::identity::InstallId;

const FINGERPRINT_DOMAIN: &str = "systemprompt-bridge-device/v1";
const DEFAULT_LABEL: &str = "bridge";

/// SHA-256 over the domain tag, install id and user id.
///
/// Why: `user_device_certs.fingerprint` is globally unique, so the user is
/// folded in to keep one install's fingerprint distinct per signed-in user.
pub fn device_fingerprint(install_id: &InstallId, user_id: &UserId) -> String {
    let material = format!(
        "{FINGERPRINT_DOMAIN}\n{}\n{}",
        install_id.as_str(),
        user_id.as_str()
    );
    crate::hash::sha256_hex(material.as_bytes())
}

/// Return the stored enrolment for `user_id`, or enrol this install with the
/// gateway and persist the resulting credential under the metadata root.
///
/// With `force_rotate` the gateway is always asked for a fresh credential;
/// the stored `installation_id` survives when the device and consumer match.
pub async fn ensure_self_enrolled(
    client: &GatewayClient,
    bearer: &BearerToken,
    install_id: &InstallId,
    user_id: &UserId,
    force_rotate: bool,
) -> Result<Enrollment> {
    let root = super::metadata_root()?;
    enroll_into(&root, client, bearer, install_id, user_id, force_rotate).await
}

/// [`ensure_self_enrolled`] against an explicit metadata root (test seam).
pub async fn enroll_into(
    root: &Path,
    client: &GatewayClient,
    bearer: &BearerToken,
    install_id: &InstallId,
    user_id: &UserId,
    force_rotate: bool,
) -> Result<Enrollment> {
    let gateway = client.base_url_str();
    let previous = Enrollment::load(root, gateway).ok();
    if !force_rotate
        && let Some(existing) = previous
            .as_ref()
            .filter(|existing| existing.consumer_id == *user_id)
    {
        return Ok(existing.clone());
    }

    let request = SelfEnrollRequest {
        fingerprint: device_fingerprint(install_id, user_id),
        label: crate::cli::login::default_device_name().unwrap_or_else(|| DEFAULT_LABEL.to_owned()),
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
