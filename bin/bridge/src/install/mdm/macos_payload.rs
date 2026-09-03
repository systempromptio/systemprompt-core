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

fn loopback_api_key(loopback: &crate::proxy::LoopbackEndpoint) -> String {
    loopback
        .secret()
        .map(crate::ids::LoopbackSecret::into_inner)
        .unwrap_or_default()
}

fn egress_plist_block(from_flag: Option<&[String]>, indent: &str) -> String {
    super::egress::cowork_egress_allowed_hosts(from_flag)
        .map(|hosts| super::egress::macos_plist_block(&hosts, indent))
        .unwrap_or_default()
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
pub fn build_prefs_plist(mcp: &MdmPayloadInputs<'_>, gateway: &str) -> String {
    PREFS_PLIST_TMPL
        .replace("{gateway_esc}", &xml::escape(gateway))
        .replace(
            "{api_key_esc}",
            &xml::escape(&loopback_api_key(mcp.loopback)),
        )
        .replace(
            "{egress_block}",
            &egress_plist_block(mcp.egress_allowed_hosts, "  "),
        )
        .replace("{managed_mcp_block}", &managed_mcp_plist_block(mcp))
}

#[must_use]
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
        .replace("{gateway_esc}", &xml::escape(gateway))
        .replace(
            "{api_key_esc}",
            &xml::escape(&loopback_api_key(mcp.loopback)),
        )
        .replace(
            "{egress_block}",
            &egress_plist_block(mcp.egress_allowed_hosts, "      "),
        )
        .replace("{managed_mcp_block}", &managed_mcp_plist_block(mcp))
        .replace("{bridge_payload}", &bridge_payload)
}

// Why: Cowork reads an empty `<dict/>` under `oauth` as "needs OAuth, do
// well-known discovery"; omitting the key disables discovery entirely.
fn managed_mcp_plist_block(mcp: &MdmPayloadInputs<'_>) -> String {
    let MdmPayloadInputs { registry, .. } = *mcp;
    if registry.is_empty() {
        return String::new();
    }
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();

    let mut out = String::new();
    out.push_str("  <key>managedMcpServers</key>\n");
    out.push_str("  <array>\n");
    for slug in slugs {
        let Some(upstream) = registry.get(slug) else {
            continue;
        };
        out.push_str("    <dict>\n");
        out.push_str(&format!(
            "      <key>name</key><string>{}</string>\n",
            xml::escape(slug)
        ));
        out.push_str(&format!(
            "      <key>url</key><string>{}</string>\n",
            xml::escape(upstream.url.as_str())
        ));
        out.push_str("      <key>transport</key><string>http</string>\n");
        out.push_str("      <key>oauth</key>\n");
        out.push_str("      <dict/>\n");
        out.push_str("    </dict>\n");
    }
    out.push_str("  </array>\n");
    out
}
