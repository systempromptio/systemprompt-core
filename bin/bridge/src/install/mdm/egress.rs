//! Cowork egress allowlist resolution for the MDM payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

const LOOPBACK_ALIAS: &str = "loopback";
const LOOPBACK_HOST: &str = "127.0.0.1";
const ENV_SUFFIX: &str = "EGRESS_ALLOWED_HOSTS";

static OVERRIDE: OnceLock<Option<Vec<String>>> = OnceLock::new();

pub fn set_egress_allowed_hosts(raw: Option<&str>) -> bool {
    OVERRIDE.set(raw.and_then(parse)).is_ok()
}

#[must_use]
pub fn cowork_egress_allowed_hosts() -> Option<Vec<String>> {
    if let Some(from_flag) = OVERRIDE.get() {
        return from_flag.clone();
    }
    std::env::var(crate::brand::brand().env(ENV_SUFFIX))
        .ok()
        .as_deref()
        .and_then(parse)
}

fn parse(raw: &str) -> Option<Vec<String>> {
    let hosts: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(|h| {
            if h.eq_ignore_ascii_case(LOOPBACK_ALIAS) {
                LOOPBACK_HOST.to_owned()
            } else {
                h.to_owned()
            }
        })
        .collect();
    (!hosts.is_empty()).then_some(hosts)
}

#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn windows_policy_value(hosts: &[String]) -> String {
    serde_json::json!(hosts).to_string()
}

#[cfg(target_os = "macos")]
#[must_use]
pub(crate) fn macos_plist_block(hosts: &[String], indent: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("{indent}<key>coworkEgressAllowedHosts</key>\n"));
    out.push_str(&format!("{indent}<array>\n"));
    for host in hosts {
        out.push_str(&format!(
            "{indent}  <string>{}</string>\n",
            crate::install::xml::escape(host)
        ));
    }
    out.push_str(&format!("{indent}</array>\n"));
    out
}
