//! What the proxy records about an upstream answer: activity on success, a
//! warning on a non-2xx, and plugin-token invalidation on a 401.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use hyper::StatusCode;
use systemprompt_identifiers::ValidatedUrl;

use crate::proxy::session::SessionContext;
use crate::proxy::usage;

pub(super) struct UpstreamStatus<'a> {
    pub(super) status: StatusCode,
    pub(super) request_path: &'a str,
    pub(super) upstream: &'a str,
    pub(super) session_context: &'a SessionContext,
    pub(super) plugin_tokens: &'a crate::auth::plugin_oauth::PluginTokenCache,
    pub(super) gateway_base: &'a ValidatedUrl,
    pub(super) hook_plugin: Option<&'a crate::ids::PluginId>,
    pub(super) gateway_signed: bool,
}

pub(super) fn note_upstream_status(seen: &UpstreamStatus<'_>) {
    let &UpstreamStatus {
        status,
        request_path,
        upstream,
        session_context,
        plugin_tokens,
        gateway_base,
        hook_plugin,
        gateway_signed,
    } = seen;
    if status.is_success() {
        if usage::is_messages_path(request_path) {
            session_context.touch_activity();
        }
        tracing::debug!(upstream_status = status.as_u16(), "upstream forwarded");
    } else {
        if systemprompt_models::ApiPaths::is_otel(request_path) {
            tracing::debug!(upstream_status = status.as_u16(), url = %upstream, "telemetry upstream non-2xx; dropped");
        } else {
            tracing::warn!(upstream_status = status.as_u16(), url = %upstream, "upstream non-2xx");
        }
        if status == StatusCode::UNAUTHORIZED {
            note_unauthorized(
                plugin_tokens,
                gateway_base,
                hook_plugin,
                gateway_signed,
                upstream,
            );
        }
    }
}

fn note_unauthorized(
    plugin_tokens: &crate::auth::plugin_oauth::PluginTokenCache,
    gateway_base: &ValidatedUrl,
    hook_plugin: Option<&crate::ids::PluginId>,
    gateway_signed: bool,
    upstream: &str,
) {
    if let Some(plugin_id) = hook_plugin {
        plugin_tokens.invalidate(gateway_base.as_str(), plugin_id);
    } else if !gateway_signed {
        // Why: only the gateway that minted the credential can say it
        // is bad. A managed MCP upstream elsewhere (or a stale entry
        // for a previous gateway) rejecting it is neither a sign-out
        // nor a reason to re-mint.
        tracing::warn!(
            upstream = %upstream,
            gateway = %gateway_base,
            "401 from a non-gateway upstream; the gateway token is kept"
        );
    }
}
