//! Rendering of the macOS managed-preferences plist and the `.mobileconfig`
//! profile from their templates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use super::MdmPayloadInputs;
use super::macos::{BRIDGE_PAYLOAD_IDENTIFIER, INNER_PAYLOAD_IDENTIFIER, PAYLOAD_IDENTIFIER};
use crate::install::xml;

const PREFS_PLIST_TMPL: &str = include_str!("../templates/prefs.plist.tmpl");
const BRIDGE_PREFS_PLIST_TMPL: &str = include_str!("../templates/bridge_prefs.plist.tmpl");
const MOBILECONFIG_TMPL: &str = include_str!("../templates/mobileconfig.tmpl");
const MOBILECONFIG_BRIDGE_PAYLOAD_TMPL: &str =
    include_str!("../templates/mobileconfig_bridge_payload.tmpl");

fn policy_body(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    indent: &str,
) -> Result<String, super::MdmError> {
    let api_key = mcp.loopback.secret().map_err(|e| {
        super::MdmError::Store(crate::config::store::ConfigStoreError::Backend(
            e.to_string(),
        ))
    })?;
    let servers = super::policy::mcp_entries(mcp.loopback, mcp.registry).map_err(|e| {
        super::MdmError::Store(crate::config::store::ConfigStoreError::Backend(
            e.to_string(),
        ))
    })?;
    let existing_models = mcp
        .policy_store
        .backend()
        .read_managed_policy("inferenceModels")?;
    let policy = super::policy::claude_desktop_policy(&super::policy::PolicyInputs {
        base_url: gateway,
        api_key: api_key.as_str(),
        models: existing_models,
        headers: &std::collections::BTreeMap::new(),
        egress_allowed_hosts: mcp.egress_allowed_hosts,
        org_uuid: crate::config::load()?
            .deployment_organization_uuid
            .as_deref(),
        mcp_servers: &servers,
    });
    Ok(super::policy::plist_body(&policy, indent))
}

pub fn build_prefs_plist(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
) -> Result<String, super::MdmError> {
    Ok(PREFS_PLIST_TMPL.replace("{policy_body}", &policy_body(mcp, gateway, "  ")?))
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_bridge_prefs_plist(pubkey: &str) -> Result<String, super::MdmError> {
    let values = super::bridge_policy_values(
        Some(pubkey),
        &crate::config::gateway_url_or_default(&crate::config::load()?),
    )?;
    Ok(BRIDGE_PREFS_PLIST_TMPL.replace("{pubkey}", &xml::escape(&values[0].2)))
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_mobileconfig(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<String, super::MdmError> {
    let bridge_payload = pubkey
        .map(|pk| -> Result<String, super::MdmError> {
            let values = super::bridge_policy_values(
                Some(pk),
                &crate::config::gateway_url_or_default(&crate::config::load()?),
            )?;
            let domain = crate::config::store::bridge_policy_domain();
            Ok(MOBILECONFIG_BRIDGE_PAYLOAD_TMPL
                .replace("{bridge_domain}", &xml::escape(&domain))
                .replace("{bridge_payload_identifier}", BRIDGE_PAYLOAD_IDENTIFIER)
                .replace(
                    "{bridge_uuid}",
                    &xml::stable_uuid(BRIDGE_PAYLOAD_IDENTIFIER),
                )
                .replace("{pubkey}", &xml::escape(&values[0].2)))
        })
        .transpose()?
        .unwrap_or_default();
    Ok(MOBILECONFIG_TMPL
        .replace("{inner_payload_identifier}", INNER_PAYLOAD_IDENTIFIER)
        .replace("{outer_payload_identifier}", PAYLOAD_IDENTIFIER)
        .replace("{inner_uuid}", &xml::stable_uuid(INNER_PAYLOAD_IDENTIFIER))
        .replace("{outer_uuid}", &xml::stable_uuid(PAYLOAD_IDENTIFIER))
        .replace("{policy_body}", &policy_body(mcp, gateway, "      ")?)
        .replace("{bridge_payload}", &bridge_payload))
}
