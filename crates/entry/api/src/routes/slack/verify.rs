//! Request authentication for the Slack inbound surface: app resolution from
//! config and HMAC signature verification over the raw request body.
//!
//! The signing secret and bot token resolve on demand from the profile secret
//! store (the MCP-registry pattern). [`verify_any_app`] exists because the
//! `url_verification` handshake carries no workspace id, so it must succeed
//! against any configured app's secret.

use axum::http::HeaderMap;
use systemprompt_config::SecretsBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::SlackAppConfig;
use systemprompt_slack::signature::verify_slack_signature;

pub(super) fn resolve_app(workspace_id: &str) -> Option<SlackAppConfig> {
    let config = ConfigLoader::load().ok()?;
    config
        .slack_apps
        .into_values()
        .find(|app| app.enabled && app.workspace_id.as_str() == workspace_id)
}

fn signing_secret(app: &SlackAppConfig) -> Option<String> {
    SecretsBootstrap::get()
        .ok()?
        .get(app.signing_secret_ref.as_str())
        .cloned()
}

pub(super) fn bot_token(app: &SlackAppConfig) -> Option<String> {
    SecretsBootstrap::get()
        .ok()?
        .get(app.bot_token_ref.as_str())
        .cloned()
}

pub(super) fn verify_app(app: &SlackAppConfig, headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(secret) = signing_secret(app) else {
        return false;
    };
    verify_signature(headers, body, secret.as_bytes())
}

pub(super) fn verify_any_app(headers: &HeaderMap, body: &[u8]) -> bool {
    let Ok(config) = ConfigLoader::load() else {
        return false;
    };
    config
        .slack_apps
        .values()
        .filter(|app| app.enabled)
        .filter_map(signing_secret)
        .any(|secret| verify_signature(headers, body, secret.as_bytes()))
}

fn verify_signature(headers: &HeaderMap, body: &[u8], secret: &[u8]) -> bool {
    let timestamp = header_str(headers, "x-slack-request-timestamp");
    let signature = header_str(headers, "x-slack-signature");
    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return false;
    };
    verify_slack_signature(secret, timestamp, signature, body, now_unix()).is_ok()
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}
