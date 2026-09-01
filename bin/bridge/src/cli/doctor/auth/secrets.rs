//! Doctor checks for the loopback secret and installed host profiles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::proxy::secret as proxy_secret;

use crate::cli::doctor::Check;

pub fn check_loopback_secret() -> Check {
    let Some(path) = proxy_secret::secret_path() else {
        return Check::fail(
            "loopback secret",
            "no config dir resolvable (crate::basedirs::config_dir() returned None)",
        );
    };
    match proxy_secret::load(&path) {
        Ok(Some(_)) => Check::ok("loopback secret", format!("{} present", path.display())),
        Ok(None) => Check::warn(
            "loopback secret",
            format!(
                "{} not present — proxy will mint it on first start; {}",
                path.display(),
                proxy_secret::reapply_hint()
            ),
        ),
        Err(e) => Check::fail("loopback secret", format!("{}: {e}", path.display())),
    }
}

#[must_use]
pub fn check_host_profile_secrets(env: &crate::integration::host_app::ProbeEnv) -> Option<Check> {
    use crate::integration::ProfileState;

    use crate::integration::StaleReason;

    let mut stale: Vec<&'static str> = Vec::new();
    let mut wrong_port: Vec<&'static str> = Vec::new();
    let mut any_installed = false;
    for host in crate::integration::host_apps() {
        match host.probe(env).profile_state {
            ProfileState::Stale {
                reason: StaleReason::LoopbackSecret,
            } => stale.push(host.display_name()),
            ProfileState::Stale {
                reason: StaleReason::ProxyPort,
            } => wrong_port.push(host.display_name()),
            ProfileState::Installed => any_installed = true,
            ProfileState::Partial { .. } | ProfileState::Absent => {},
        }
    }
    if !wrong_port.is_empty() {
        return Some(Check::fail(
            "host profile secret",
            format!(
                "{} points at a proxy port this install no longer holds (the proxy is on {}); {}",
                wrong_port.join(", "),
                env.proxy_port,
                proxy_secret::reapply_hint()
            ),
        ));
    }
    if !stale.is_empty() {
        return Some(Check::fail(
            "host profile secret",
            format!(
                "{} carries an out-of-date loopback secret (installed fingerprint ≠ live \
                 secret); {}",
                stale.join(", "),
                proxy_secret::reapply_hint()
            ),
        ));
    }
    any_installed.then(|| {
        Check::ok(
            "host profile secret",
            "installed host profiles match the live loopback secret",
        )
    })
}
