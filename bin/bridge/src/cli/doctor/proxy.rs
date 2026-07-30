//! Doctor checks for the loopback inference proxy and, on Linux, the systemd
//! user unit that keeps it running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::integration::proxy_probe::{self, ProxyProbeState};

use super::Check;

/// A proxy that is not listening is the single most common cause of "Claude
/// Code cannot reach the gateway", so it is reported as a warning with the
/// command that fixes it rather than left to be inferred.
#[must_use]
pub fn check_proxy_listening() -> Check {
    let url = crate::proxy::loopback_origin();
    let health = proxy_probe::probe(Some(&url));
    let bin = crate::brand::brand().binary_name;
    match health.state {
        ProxyProbeState::Listening => Check::ok(
            "inference proxy",
            format!(
                "{url} responding ({}ms)",
                health.latency_ms.unwrap_or_default()
            ),
        ),
        ProxyProbeState::Refused => Check::warn(
            "inference proxy",
            format!("nothing listening on {url} — start it with `{bin} proxy`"),
        ),
        other => Check::warn(
            "inference proxy",
            format!(
                "{url} probe returned {other:?}{}",
                health.error.map(|e| format!(": {e}")).unwrap_or_default()
            ),
        ),
    }
}

/// Linux-only: the proxy has no GUI to own its lifecycle, so the systemd user
/// service written by `install --apply-schedule` is what survives a reboot.
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
