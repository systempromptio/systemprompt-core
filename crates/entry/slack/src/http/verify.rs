//! Request authentication for the Slack inbound surface: app resolution from
//! config and HMAC signature verification over the raw request body. A
//! verified request yields a `SlackVerified` proof, which nothing outside
//! this module can build and which is the one thing the member's dispatch
//! handle accepts beside the inbound message.
//!
//! The signing secret and bot token resolve on demand by their named refs
//! through the member's [`SlackSecrets`]; config validation refuses a ref
//! that names a built-in platform secret. [`verify_any_app`] exists
//! because the `url_verification` handshake carries no workspace id, so it must
//! succeed against any configured app's secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::SlackWorkspaceId;
use systemprompt_manifest::services::{ServicesConfig, SlackAppConfig};
use systemprompt_slack::signature::verify_slack_signature;

use super::state::SlackSecrets;

pub(super) fn resolve_app(
    services: &ServicesConfig,
    workspace_id: &SlackWorkspaceId,
) -> Option<SlackAppConfig> {
    services
        .slack_apps
        .values()
        .find(|app| app.enabled && app.workspace_id == *workspace_id)
        .cloned()
}

#[derive(Debug)]
pub(crate) struct SlackVerified {
    _verified: (),
}

pub(super) fn verify_app(
    secrets: &SlackSecrets,
    app: &SlackAppConfig,
    headers: &HeaderMap,
    body: &[u8],
) -> Option<SlackVerified> {
    let secret = secrets.signing_secret(app)?;
    verify_signature(headers, body, secret.as_bytes()).then_some(SlackVerified { _verified: () })
}

pub(super) fn verify_any_app(
    secrets: &SlackSecrets,
    services: &ServicesConfig,
    headers: &HeaderMap,
    body: &[u8],
) -> bool {
    services
        .slack_apps
        .values()
        .filter(|app| app.enabled)
        .filter_map(|app| secrets.signing_secret(app))
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
