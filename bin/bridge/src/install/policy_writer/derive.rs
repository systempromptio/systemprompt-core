//! Policy derivation and trust-anchor verification for a writer request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::manifest::SignedManifest;

use systemprompt_models::bridge::host::HostKind;

use super::{MachineBinding, PolicyWriteRequest, PolicyWriterError};
use crate::install::mdm::MdmError;

// Why: the account, port and token are the binding's; the servers, their
// tool policy and the inference headers are the signed manifest's. The
// model list and the organisation are not in the manifest, so the writer
// keeps the ones the elevated install put in the hive (`existing_models`)
// and writes no organisation value, which leaves the installed one in place.
pub fn derive_policy(
    binding: &MachineBinding,
    request: &PolicyWriteRequest,
    manifest: &SignedManifest,
    existing_models: Option<String>,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    let loopback = crate::proxy::LoopbackEndpoint::new(binding.loopback_port, None);
    let headers = crate::install::profile_inputs::inference_headers(
        HostKind::ClaudeDesktop,
        &manifest.host_model_protocols,
    );
    let registry = crate::mcp_registry::from_servers(&manifest.managed_mcp_servers);
    let servers = crate::install::mdm::policy_mcp::mcp_entries_with(
        &loopback,
        &registry,
        &request.tool_catalog,
        None,
    );
    let policy = crate::install::mdm::policy::claude_desktop_policy_with(
        &crate::install::mdm::policy::PolicyInputs {
            base_url: &loopback.origin(),
            host_token: &binding.host_token,
            models: existing_models,
            headers: &headers,
            egress_allowed_hosts: None,
            org_uuid: None,
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
