//! Doctor checks for stored credentials and gateway auth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{PluginId, SessionId};

use crate::auth::{self, ChainError, plugin_oauth};
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::proxy::secret as proxy_secret;

use super::Check;

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

pub async fn check_mint_jwt(
    cfg: &config::Config,
    checks: &mut Vec<Check>,
) -> Option<auth::types::HelperOutput> {
    let session_id = SessionId::generate();
    match auth::acquire_bearer(cfg, &session_id).await {
        Ok(out) => {
            checks.push(Check::ok(
                "mint JWT",
                "auth chain succeeded; gateway accepted credentials at token-exchange",
            ));
            Some(out)
        },
        Err(ChainError::PreferredTransient { provider, source }) => {
            checks.push(Check::fail(
                "mint JWT",
                format!("preferred provider `{provider}` failed transiently: {source}"),
            ));
            None
        },
        Err(ChainError::NoneSucceeded) => {
            checks.push(Check::fail(
                "mint JWT",
                format!(
                    "no provider in the chain succeeded — run `{} login`",
                    crate::brand::brand().binary_name
                ),
            ));
            None
        },
    }
}

pub async fn check_gateway_reachable(
    cfg: &config::Config,
    checks: &mut Vec<Check>,
) -> GatewayClient {
    let gateway = config::gateway_url_or_default(cfg);
    let client = GatewayClient::new(gateway.clone());
    match client.health().await {
        Ok(()) => checks.push(Check::ok(
            "gateway reachable",
            format!("{} responds on /health", gateway.as_str()),
        )),
        Err(e) => checks.push(Check::fail(
            "gateway reachable",
            format!("{}: {e}", gateway.as_str()),
        )),
    }
    client
}

pub async fn check_whoami(
    client: &GatewayClient,
    bearer: Option<&auth::types::HelperOutput>,
    checks: &mut Vec<Check>,
) {
    let Some(out) = bearer else {
        checks.push(Check::fail(
            "authenticated whoami",
            "skipped: no bearer token available (see `mint JWT` above)",
        ));
        return;
    };
    match client.fetch_whoami(out.token.expose()).await {
        Ok(_) => checks.push(Check::ok(
            "authenticated whoami",
            "GET /v1/bridge/whoami returned identity",
        )),
        Err(GatewayError::HttpStatus { status, endpoint }) if status.as_u16() == 401 => {
            checks.push(Check::fail(
                "authenticated whoami",
                format!(
                    "{endpoint} returned 401 — the PAT is invalid or revoked; mint a new one and \
                     re-run `{} login`",
                    crate::brand::brand().binary_name
                ),
            ));
        },
        Err(e) => checks.push(Check::fail(
            "authenticated whoami",
            format!("whoami failed: {e}"),
        )),
    }
}

pub fn check_loopback_secret() -> Check {
    let Some(path) = proxy_secret::secret_path() else {
        return Check::fail(
            "loopback secret",
            "no config dir resolvable (dirs::config_dir() returned None)",
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

/// A baked-vs-live loopback secret mismatch is the silent `403 bad loopback
/// secret` failure mode this surfaces.
#[must_use]
pub fn check_host_profile_secrets() -> Option<Check> {
    use crate::integration::ProfileState;

    let mut stale: Vec<&'static str> = Vec::new();
    let mut any_installed = false;
    for host in crate::integration::host_apps() {
        match host.probe().profile_state {
            ProfileState::Stale => stale.push(host.display_name()),
            ProfileState::Installed => any_installed = true,
            ProfileState::Partial { .. } | ProfileState::Absent => {},
        }
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

pub fn check_pinned_pubkey() -> Check {
    if config::pinned_pubkey().is_some() {
        Check::ok(
            "manifest pubkey pinned",
            "signed-manifest verification will reject pubkey rotation",
        )
    } else {
        Check::warn(
            "manifest pubkey pinned",
            "no pinned pubkey — first sync needs `--allow-tofu` or `install --apply --pubkey \
             <b64>`",
        )
    }
}

// Surfaces hook-token mint errors that otherwise fail silently as a
// host_failures row in `sync` PARTIAL output.
pub async fn check_hook_token_mint(gateway: &GatewayClient) -> Check {
    let creds = match plugin_oauth::load_creds() {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Check::warn(
                "hook token mint",
                "no bridge OAuth client provisioned yet — runs on first sync after login",
            );
        },
        Err(e) => {
            return Check::fail("hook token mint", format!("load OAuth client creds: {e}"));
        },
    };
    let plugin_id = PluginId::new("__doctor__");
    match gateway
        .mint_plugin_hook_token(
            &creds.token_endpoint,
            &creds.client_id,
            &creds.client_secret,
            &plugin_id,
        )
        .await
    {
        Ok(_) => Check::ok(
            "hook token mint",
            format!(
                "{} accepted hook:govern hook:track for client {}",
                creds.token_endpoint,
                creds.client_id.as_str()
            ),
        ),
        Err(GatewayError::HookTokenRejected { status, body }) => Check::fail(
            "hook token mint",
            format!(
                "gateway rejected hook token: status={status} body={body} — operator action: \
                 confirm the bridge OAuth client grants `hook:govern hook:track` and that \
                 service-tier scopes are not being intersected with owner roles",
            ),
        ),
        Err(e) => Check::fail("hook token mint", format!("mint failed: {e}")),
    }
}
