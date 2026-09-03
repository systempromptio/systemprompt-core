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

fn policy_body(mcp: &MdmPayloadInputs<'_>, gateway: &str, indent: &str) -> String {
    let api_key = mcp
        .loopback
        .secret()
        .map(crate::ids::LoopbackSecret::into_inner)
        .unwrap_or_default();
    // Why: a registry that cannot be resolved into loopback entries (no
    // secret yet) publishes an empty list rather than servers with no
    // credential — the shape that could never authenticate.
    let servers = super::policy::mcp_entries(mcp.loopback, mcp.registry).unwrap_or_else(|e| {
        tracing::warn!(
            target: "bridge::install::mdm",
            error = %e,
            "loopback secret unavailable; publishing an empty managed MCP server list"
        );
        Vec::new()
    });
    let existing_models = crate::config::store::managed_policy_store()
        .read_managed_policy("inferenceModels")
        .ok()
        .flatten();
    let policy = super::policy::claude_desktop_policy(&super::policy::PolicyInputs {
        base_url: gateway,
        api_key: &api_key,
        models: existing_models,
        headers: &std::collections::BTreeMap::new(),
        egress_allowed_hosts: mcp.egress_allowed_hosts,
        org_uuid: crate::config::load()
            .deployment_organization_uuid
            .as_deref(),
        mcp_servers: &servers,
    });
    super::policy::plist_body(&policy, indent)
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_prefs_plist(mcp: &MdmPayloadInputs<'_>, gateway: &str) -> String {
    PREFS_PLIST_TMPL.replace("{policy_body}", &policy_body(mcp, gateway, "  "))
}

#[must_use]
#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_bridge_prefs_plist(pubkey: &str) -> String {
    BRIDGE_PREFS_PLIST_TMPL.replace("{pubkey}", &xml::escape(pubkey))
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_mobileconfig(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> String {
    let bridge_payload = pubkey
        .map(|pk| {
            let domain = crate::config::store::bridge_policy_domain();
            MOBILECONFIG_BRIDGE_PAYLOAD_TMPL
                .replace("{bridge_domain}", &xml::escape(&domain))
                .replace("{bridge_payload_identifier}", BRIDGE_PAYLOAD_IDENTIFIER)
                .replace(
                    "{bridge_uuid}",
                    &xml::stable_uuid(BRIDGE_PAYLOAD_IDENTIFIER),
                )
                .replace("{pubkey}", &xml::escape(pk))
        })
        .unwrap_or_default();
    MOBILECONFIG_TMPL
        .replace("{inner_payload_identifier}", INNER_PAYLOAD_IDENTIFIER)
        .replace("{outer_payload_identifier}", PAYLOAD_IDENTIFIER)
        .replace("{inner_uuid}", &xml::stable_uuid(INNER_PAYLOAD_IDENTIFIER))
        .replace("{outer_uuid}", &xml::stable_uuid(PAYLOAD_IDENTIFIER))
        .replace("{policy_body}", &policy_body(mcp, gateway, "      "))
        .replace("{bridge_payload}", &bridge_payload)
}
