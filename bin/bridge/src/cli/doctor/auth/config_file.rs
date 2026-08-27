//! Doctor checks for the on-disk config and install record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::{auth, config};

use crate::cli::doctor::Check;

pub fn check_config_file() -> Check {
    let Some(path) = config::config_path() else {
        return Check::fail("config file", "no config dir resolvable");
    };
    if !path.exists() {
        return Check::warn(
            "config file",
            format!(
                "{} not present — defaults will be used; run `{} login` to \
                 create it",
                path.display(),
                crate::brand::brand().binary_name
            ),
        );
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<toml::Value>(&text) {
            Ok(_) => Check::ok("config file", format!("{} parses cleanly", path.display())),
            Err(e) => Check::fail(
                "config file",
                format!("{}: parse error: {e}", path.display()),
            ),
        },
        Err(e) => Check::fail("config file", format!("{}: {e}", path.display())),
    }
}

pub fn check_install_record(cfg: &config::Config) -> Check {
    let Some(record) = crate::install::bootstrap::read_install_record() else {
        return Check::warn(
            "install record",
            format!(
                "no install record — run `{} install --apply` so host launchers can be \
                 checked against a known binary",
                crate::brand::brand().binary_name
            ),
        );
    };
    let running = std::env::current_exe().map_or_else(
        |_| String::from("<unresolvable>"),
        |p| p.display().to_string(),
    );
    let local = crate::brand::brand().version;
    let configured = config::gateway_url_or_default(cfg);

    if record.binary_version != local {
        return Check::warn(
            "install record",
            format!(
                "hosts were wired to {} {} but this process is {local} — re-run `{} install \
                 --apply` so they launch the current binary",
                record.binary,
                record.binary_version,
                crate::brand::brand().binary_name
            ),
        );
    }
    if record.binary != running {
        return Check::warn(
            "install record",
            format!(
                "hosts launch {} but this process is {running}",
                record.binary
            ),
        );
    }
    if record
        .gateway_url
        .as_deref()
        .is_some_and(|g| g != configured.as_str())
    {
        return Check::warn(
            "install record",
            format!(
                "hosts were wired for gateway {} but the configured gateway is {configured}",
                record.gateway_url.unwrap_or_default()
            ),
        );
    }
    Check::ok(
        "install record",
        format!("hosts launch {} ({local})", record.binary),
    )
}

pub fn check_cached_gateway(cfg: &config::Config) -> Check {
    let configured = config::gateway_url_or_default(cfg);
    match auth::cache::cached_gateway() {
        None => Check::ok(
            "cached token scope",
            "no cached token; the next call mints against the configured gateway",
        ),
        Some(cached) if cached == configured => Check::ok(
            "cached token scope",
            format!("cached token was minted for {configured}"),
        ),
        Some(cached) => Check::warn(
            "cached token scope",
            format!(
                "cached token was minted for {cached} but the configured gateway is \
                 {configured} — it will be discarded and re-minted on the next call"
            ),
        ),
    }
}

pub fn check_credential_source(cfg: &config::Config) -> Check {
    if auth::has_credential_source(cfg) {
        Check::ok(
            "credential source",
            "at least one auth provider is configured (PAT, session, or mTLS)",
        )
    } else {
        Check::fail(
            "credential source",
            format!(
                "no auth provider configured — run `{} login <sp-live-...>`",
                crate::brand::brand().binary_name
            ),
        )
    }
}
