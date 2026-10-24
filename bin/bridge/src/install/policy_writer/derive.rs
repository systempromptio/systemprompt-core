//! Policy derivation and trust-anchor verification for a writer request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::manifest::SignedManifest;

use super::{PolicyWriteRequest, PolicyWriterError};
use crate::install::mdm::MdmError;

pub fn derive_policy(
    request: &PolicyWriteRequest,
    manifest: &SignedManifest,
    existing_models: Option<String>,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    let loopback = crate::proxy::LoopbackEndpoint::new(request.loopback_port, None);
    let registry = crate::mcp_registry::from_servers(&manifest.managed_mcp_servers);
    let servers = crate::install::mdm::policy_mcp::mcp_entries_with(
        &loopback,
        &registry,
        &request.tool_catalog,
    );
    let policy = crate::install::mdm::policy::claude_desktop_policy_with(
        &crate::install::mdm::policy::PolicyInputs {
            base_url: &loopback.origin(),
            host_token: &request.host_token,
            models: request.models.clone().or(existing_models),
            headers: &request.headers,
            egress_allowed_hosts: None,
            org_uuid: request.org_uuid.as_deref(),
            mcp_servers: servers.as_deref(),
        },
        &manifest.desktop_policy,
    )?;
    Ok(crate::install::mdm::policy::reg_values(&policy))
}

pub fn verify_against_anchor(
    request: &PolicyWriteRequest,
    anchor: &crate::config::TrustRecord,
) -> Result<SignedManifest, PolicyWriterError> {
    let requested = systemprompt_identifiers::ValidatedUrl::try_new(&request.gateway)
        .map_err(|e| PolicyWriterError::Anchor(crate::config::TrustError::GatewayInvalid(e)))?;
    let requested = crate::config::GatewayIdentity::new(&requested)?;
    if requested != anchor.gateway {
        return Err(PolicyWriterError::GatewayMismatch {
            requested: requested.to_string(),
            anchored: anchor.gateway.to_string(),
        });
    }
    crate::gateway::manifest::verify_envelope(&request.envelope, anchor.key.as_str())?;
    Ok(crate::gateway::manifest::decode_payload(&request.envelope)?)
}
