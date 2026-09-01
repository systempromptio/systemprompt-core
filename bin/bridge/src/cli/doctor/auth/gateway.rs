//! Doctor checks that exercise the gateway and token minting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{PluginId, SessionId};

use crate::auth::{self, ChainError, plugin_oauth};
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;

use crate::cli::doctor::Check;

pub async fn check_mint_jwt(
    cfg: &config::Config,
    checks: &mut Vec<Check>,
    http: &reqwest::Client,
) -> Option<crate::gateway::types::HelperOutput> {
    let session_id = SessionId::generate();
    match auth::acquire_bearer(cfg, &session_id, http).await {
        Ok(out) => {
            checks.push(Check::ok(
                "mint JWT",
                "auth chain produced a bearer token — see `authenticated whoami` below for \
                 whether the gateway actually accepts it",
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
    http: &reqwest::Client,
) -> GatewayClient {
    let gateway = config::gateway_url_or_default(cfg);
    let client = GatewayClient::new(gateway.clone(), http.clone());
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
    bearer: Option<&crate::gateway::types::HelperOutput>,
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

pub fn check_credential_store() -> Check {
    match plugin_oauth::credential_backend() {
        plugin_oauth::SecretBackend::Keyring => Check::ok(
            "credential store",
            "OS credential store available for the OAuth client secret",
        ),
        plugin_oauth::SecretBackend::Memory => Check::warn(
            "credential store",
            "no OS credential store (no Secret Service provider, and the kernel keyutils keyring \
             is unavailable — Docker's default seccomp profile denies it). The OAuth client \
             secret is held in the proxy's memory and re-provisioned on restart, so hooks work \
             but nothing persists. Install gnome-keyring, or run with \
             `--security-opt seccomp=unconfined`.",
        ),
    }
}

pub async fn check_hook_token_mint(gateway: &GatewayClient) -> Check {
    let creds = match plugin_oauth::load_creds() {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Check::warn(
                "hook token mint",
                "no bridge OAuth client provisioned yet — provisioning is lazy, on the first \
                 plugin hook request, not during sync. On the in-memory credential backend a \
                 separate process legitimately sees none; check `credential store` above.",
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
