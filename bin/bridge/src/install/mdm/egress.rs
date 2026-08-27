//! Cowork egress allowlist resolution for the MDM payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

/// Expands to loopback-only egress, the pre-0.29 hard-coded default.
const LOOPBACK_ALIAS: &str = "loopback";
const LOOPBACK_HOST: &str = "127.0.0.1";
const ENV_SUFFIX: &str = "EGRESS_ALLOWED_HOSTS";

static OVERRIDE: OnceLock<Option<Vec<String>>> = OnceLock::new();

/// Records the `--egress-allowed-hosts` value parsed from the command line.
///
/// Returns `false` if an override was already recorded. Only the first call
/// wins, matching [`crate::brand::set_brand`].
pub fn set_egress_allowed_hosts(raw: Option<&str>) -> bool {
    OVERRIDE.set(raw.and_then(parse)).is_ok()
}

/// Hosts Cowork may reach.
///
/// `None` means the policy key is omitted entirely, which leaves Cowork's own
/// default — unrestricted egress — in force. That is the shipping default:
/// pinning the allowlist to loopback left agents with no internet access at
/// all, which is a deliberate lockdown rather than something a stock install
/// should inherit. Regulated deployments opt back in with
/// `--egress-allowed-hosts loopback` or the environment variable.
///
/// Resolution order: `--egress-allowed-hosts` → `<PREFIX>_EGRESS_ALLOWED_HOSTS`
/// → `None`.
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

/// Splits a comma-separated host list, expanding the `loopback` alias.
///
/// An empty or all-whitespace value resolves to `None` so that setting the
/// variable to the empty string is a way to say "no restriction", not "an
/// empty allowlist that blocks everything".
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

/// Renders the allowlist as the JSON array the Windows registry value holds.
#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn windows_policy_value(hosts: &[String]) -> String {
    serde_json::json!(hosts).to_string()
}

/// Renders the allowlist as the plist `<array>` block both macOS templates
/// substitute for `{egress_block}`.
///
/// Indentation is passed in because the key sits two levels deeper inside the
/// mobileconfig payload than it does in the bare managed-preferences plist.
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
