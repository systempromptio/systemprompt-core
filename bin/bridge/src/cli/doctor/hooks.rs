//! Checks that the hook URLs already written into host plugin trees still name
//! the port the proxy holds.
//!
//! [`super::proxy::check_proxy_client_config`] only sees a host's profile keys.
//! Hook URLs are baked separately, into each mirrored `hooks/hooks.json`, and a
//! proxy that moved ports leaves them pointing somewhere nothing answers — the
//! `ECONNREFUSED` an agent host reports on every single tool call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::proxy_probe::{self, PortMatch};

use super::Check;

#[must_use]
pub fn check_hook_urls(actual: u16) -> Option<Check> {
    let bin = crate::brand::brand().binary_name;
    let files = hook_files()?;
    if files.is_empty() {
        return None;
    }

    let mut urls = Vec::new();
    for file in &files {
        if let Ok(text) = std::fs::read_to_string(file) {
            urls.extend(hook_urls_in(&text));
        }
    }
    let checked = urls.len();
    let stale = stale_ports(&urls, actual);

    if checked == 0 {
        return None;
    }
    if stale.is_empty() {
        return Some(Check::ok(
            "hook urls",
            format!("{checked} hook URLs point at 127.0.0.1:{actual}"),
        ));
    }
    let ports = stale
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Some(Check::fail(
        "hook urls",
        format!(
            "hook URLs name port(s) {ports} but the proxy is on {actual} — every tool call in the \
             agent host fails to connect. Fix with `{bin} sync`."
        ),
    ))
}

fn hook_files() -> Option<Vec<PathBuf>> {
    use crate::integration::claude_code_cli::{marketplace_dir, sidecar};
    let plugins = crate::config::paths::claude_cli_plugins_dir()?;
    let mut files = Vec::new();
    for marketplace in sidecar::owned_marketplaces(&plugins) {
        let root = marketplace_dir(&plugins, &marketplace).join("plugins");
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        files.extend(
            entries
                .flatten()
                .map(|e| e.path().join("hooks").join("hooks.json"))
                .filter(|p| p.is_file()),
        );
    }
    Some(files)
}

// Why: the host's schema nests `url` at varying depths, so this walks the
// whole value rather than reading a fixed path.
#[must_use]
pub fn hook_urls_in(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_urls(&value, &mut out);
    out
}

#[must_use]
pub fn stale_ports(urls: &[String], actual: u16) -> BTreeSet<u16> {
    urls.iter()
        .filter_map(
            |url| match proxy_probe::classify_configured_port(url, actual) {
                PortMatch::Mismatch { configured } => Some(configured),
                PortMatch::Match | PortMatch::NotLoopback | PortMatch::Unparseable => None,
            },
        )
        .collect()
}

fn collect_urls(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(url)) = map.get("url") {
                out.push(url.clone());
            }
            for nested in map.values() {
                collect_urls(nested, out);
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                collect_urls(item, out);
            }
        },
        _ => {},
    }
}
