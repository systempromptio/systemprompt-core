//! Doctor checks for the loopback inference proxy and, on Linux, the systemd
//! user unit that keeps it running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::integration::host_app::ProbeEnv;
use crate::proxy::LoopbackEndpoint;
use crate::proxy::peer::{self, PeerIdentity};
use crate::proxy_probe::{self, PortMatch};

use super::Check;

#[must_use]
pub fn check_proxy_listening(loopback: &LoopbackEndpoint) -> Check {
    let port = loopback.port();
    let url = loopback.origin();
    let bin = crate::brand::brand().binary_name;

    match peer::probe_identity(port) {
        PeerIdentity::Ours(_) => {
            let health = proxy_probe::probe(Some(&url));
            let latency = health.latency_ms.unwrap_or_default();
            if port == crate::proxy::DEFAULT_PROXY_PORT {
                Check::ok("inference proxy", format!("{url} responding ({latency}ms)"))
            } else {
                Check::warn(
                    "inference proxy",
                    format!(
                        "{url} responding ({latency}ms) — this is a fallback port; port {} was \
                         taken when the proxy started. Client config written for the default port \
                         will be rejected.",
                        crate::proxy::DEFAULT_PROXY_PORT
                    ),
                )
            }
        },
        PeerIdentity::Foreign(who) => Check::fail(
            "inference proxy",
            format!(
                "{url} is answering, but it belongs to a different {} install ({}). Requests from \
                 this install are rejected with 403. Restart one of them so they take different \
                 ports, then run `{bin} install --apply`.",
                crate::brand::brand().app_name,
                who.config_dir
            ),
        ),
        PeerIdentity::Unknown => Check::warn(
            "inference proxy",
            format!(
                "{url} is responding but did not identify itself — an unrelated service, or a \
                 bridge older than the identity endpoint"
            ),
        ),
        PeerIdentity::Unreachable => Check::warn(
            "inference proxy",
            format!("nothing listening on {url} — start it with `{bin} proxy`"),
        ),
    }
}

#[must_use]
pub fn check_proxy_client_config(env: &ProbeEnv) -> Vec<Check> {
    let actual = env.proxy_port;
    let bin = crate::brand::brand().binary_name;
    let mut checks = Vec::new();

    for host in crate::integration::registry::host_apps() {
        let snapshot = host.probe(env);
        let Some(configured) = snapshot
            .profile_keys
            .get("inferenceGatewayBaseUrl")
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        match proxy_probe::classify_configured_port(configured, actual) {
            PortMatch::Match => checks.push(Check::ok(
                "client config port",
                format!("{} points at 127.0.0.1:{actual}", host.display_name()),
            )),
            PortMatch::Mismatch { configured } => checks.push(Check::fail(
                "client config port",
                format!(
                    "{} is configured for 127.0.0.1:{configured} but the proxy is on {actual} — \
                     this produces 403 'bad loopback secret'. Fix with `{bin} install --apply`, \
                     then restart {}.",
                    host.display_name(),
                    host.display_name()
                ),
            )),
            // Why: a deliberately remote or unparseable base URL is covered by
            // the host's own profile checks.
            PortMatch::NotLoopback | PortMatch::Unparseable => {},
        }
    }
    checks
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn check_proxy_service() -> Option<Check> {
    use std::process::Command;

    let unit = format!("{}.service", crate::schedule::proxy_unit_name());
    let bin = crate::brand::brand().binary_name;

    let dir = crate::basedirs::home_dir()?
        .join(".config")
        .join("systemd")
        .join("user");
    let path = dir.join(&unit);
    if !path.exists() {
        return Some(Check::warn(
            "proxy service",
            format!(
                "{} not present — register it with `{bin} install --apply-schedule`",
                path.display()
            ),
        ));
    }

    let output = Command::new("systemctl")
        .args(["--user", "is-active", &unit])
        .output();
    let Ok(output) = output else {
        return Some(Check::warn(
            "proxy service",
            format!("{unit} written, but systemctl is unavailable to check it"),
        ));
    };
    let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if state == "active" {
        return Some(Check::ok("proxy service", format!("{unit} active")));
    }
    Some(Check::warn(
        "proxy service",
        format!(
            "{unit} is '{}' — start it with `systemctl --user enable --now {unit}`",
            if state.is_empty() { "unknown" } else { &state }
        ),
    ))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[must_use]
pub const fn check_proxy_service() -> Option<Check> {
    None
}
