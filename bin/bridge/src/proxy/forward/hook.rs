//! Plugin hook track: the credential a hook route accepts and the device
//! evidence the forwarded hook carries to the gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{ForwardError, ForwardResult};
use crate::proxy::credential::LoopbackCredential;

pub(super) fn authenticate_hook_track(
    gateway_base: &ValidatedUrl,
    request_headers: &http::HeaderMap,
    buffered_body: &[u8],
    upstream_headers: &mut http::HeaderMap,
) -> ForwardResult<()> {
    let host = request_headers
        .get("x-systemprompt-host")
        .and_then(|value| value.to_str().ok());
    if let Err(error) = crate::feedback::hooks::authenticate_forwarded_hook(
        gateway_base.as_str(),
        host,
        upstream_headers,
    ) && !matches!(error, crate::feedback::FeedbackError::EnrollmentRequired)
    {
        return Err(ForwardError::Auth(
            "Device evidence authentication unavailable".to_owned(),
        ));
    }
    // JSON: protocol boundary — the hook body is the host's own wire shape.
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(buffered_body)
        && let (Some(host), Some(session)) = (
            host.and_then(crate::feedback::client_kind),
            value.get("session_id").and_then(serde_json::Value::as_str),
        )
        && let Ok(root) = crate::feedback::metadata_root()
        && let Ok(enrollment) =
            crate::feedback::credentials::Enrollment::load(&root, gateway_base.as_str())
    {
        let outbox = crate::feedback::outbox::Outbox::new(
            enrollment.outbox_path(&root),
            crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
        );
        if let Err(error) = outbox.queue_session(host, session) {
            tracing::debug!(%error, "Hook native session awaits binding");
        }
    }
    Ok(())
}

pub(super) fn require_hook_credential(
    credential: &LoopbackCredential,
    plugin_id: &str,
) -> ForwardResult<()> {
    match credential {
        LoopbackCredential::Hook(plugin) if plugin.as_str() == plugin_id => Ok(()),
        LoopbackCredential::Hook(_) => Err(ForwardError::Scope {
            presented: "hook token of another plugin",
            route: "this plugin's hook route",
        }),
        LoopbackCredential::Secret => Err(ForwardError::Scope {
            presented: "loopback secret",
            route: "a plugin hook route",
        }),
        LoopbackCredential::Host(_) => Err(ForwardError::Scope {
            presented: "host token",
            route: "a plugin hook route",
        }),
    }
}
