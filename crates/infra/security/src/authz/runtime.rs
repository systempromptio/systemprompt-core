//! Construction of the active authz decision hook from profile config.
//!
//! [`build_authz_hook`] is the single entry point for both the API server and
//! any standalone MCP binary. It inspects `governance.authz` and returns one
//! of [`DenyAllHook`], [`AllowAllHook`], [`WebhookHook`], or an
//! extension-supplied hook, wrapped in an `Arc<dyn AuthzDecisionHook>` that
//! the caller stores on its `AppContext` (or equivalent) and threads to every
//! consumer.
//!
//! Branch table:
//!
//! - `mode: webhook` with a non-empty `url` that passes SSRF validation →
//!   [`WebhookHook`] (fail-closed). A url pointing at loopback over `http`,
//!   `169.254.169.254`, or an RFC1918 range fails bootstrap.
//! - `mode: extension` with a hook supplied via `AppContextBuilder::with_authz_hook`
//!   → that hook. Bootstrap fails with [`AuthzBootstrapError::ExtensionModeButNoHook`]
//!   if no hook was supplied.
//! - `mode: disabled`, or governance/authz absent → [`DenyAllHook`].
//! - `mode: unrestricted` → [`AllowAllHook`], but ONLY when `acknowledgement`
//!   exactly equals [`UNRESTRICTED_ACKNOWLEDGEMENT`]. Otherwise bootstrap
//!   fails. An error-level warning is always logged; refusing this mode in
//!   production is the operator's responsibility.
//! - Any other mode combined with a supplied extension hook → bootstrap fails
//!   with [`AuthzBootstrapError::ExtensionHookButWrongMode`] so an operator
//!   never silently runs the wrong mode.
//!
//! Bootstrap ordering: called from `AppContextBuilder::build` after the
//! database pool is created so the audit sink can write to
//! `governance_decisions`. The extension hook, if supplied, is constructed
//! by the binary entry point with the same pool already in scope — see
//! `internal/guides/authz.md` for the contract.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_models::net::validate_outbound_url;
use systemprompt_models::profile::{AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT};

use super::audit::{AuthzAuditSink, DbAuditSink, GovernanceDecisionRepository, NullAuditSink};
use super::error::{AuthzBootstrapError, AuthzResult};
use super::hook::{AllowAllHook, AuthzDecisionHook, DenyAllHook, WebhookHook};

pub type SharedAuthzHook = Arc<dyn AuthzDecisionHook>;

pub fn build_authz_hook(
    governance: Option<&GovernanceConfig>,
    pool: Option<Arc<sqlx::PgPool>>,
    extension: Option<SharedAuthzHook>,
) -> AuthzResult<SharedAuthzHook> {
    let sink = build_sink(pool);

    let Some(authz) = governance.and_then(|g| g.authz.as_ref()) else {
        if extension.is_some() {
            return Err(AuthzBootstrapError::ExtensionHookButWrongMode { mode: "disabled" }.into());
        }
        tracing::error!(
            "governance.authz block missing — using DenyAllHook (all requests will be denied)"
        );
        return Ok(Arc::new(DenyAllHook::new(sink)));
    };

    match (authz.hook.mode, extension) {
        (AuthzMode::Extension, Some(hook)) => {
            tracing::info!(
                "governance.authz.hook.mode = extension — using injected hook {:?}",
                hook
            );
            Ok(hook)
        },
        (AuthzMode::Extension, None) => Err(AuthzBootstrapError::ExtensionModeButNoHook.into()),
        (mode, Some(_)) => Err(AuthzBootstrapError::ExtensionHookButWrongMode {
            mode: mode_name(mode),
        }
        .into()),
        (AuthzMode::Disabled, None) => {
            tracing::warn!("governance.authz.hook.mode = disabled — all requests will be denied");
            Ok(Arc::new(DenyAllHook::new(sink)))
        },
        (AuthzMode::Unrestricted, None) => {
            let ack = authz.hook.acknowledgement.as_deref().map_or("", str::trim);
            if ack != UNRESTRICTED_ACKNOWLEDGEMENT {
                return Err(AuthzBootstrapError::MissingUnrestrictedAcknowledgement {
                    expected: UNRESTRICTED_ACKNOWLEDGEMENT,
                }
                .into());
            }
            tracing::error!(
                "governance.authz.hook.mode = unrestricted — ALL REQUESTS WILL BE ALLOWED. This \
                 MUST NOT run in production."
            );
            Ok(Arc::new(AllowAllHook::new(sink)))
        },
        (AuthzMode::Webhook, None) => {
            let url = authz
                .hook
                .url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(AuthzBootstrapError::MissingWebhookUrl)?
                .to_owned();
            validate_outbound_url(&url)
                .map_err(|e| AuthzBootstrapError::InvalidWebhookUrl(e.to_string()))?;
            let hook = WebhookHook::new(url, Duration::from_millis(authz.hook.timeout_ms), sink)?;
            Ok(Arc::new(hook))
        },
    }
}

const fn mode_name(mode: AuthzMode) -> &'static str {
    match mode {
        AuthzMode::Webhook => "webhook",
        AuthzMode::Extension => "extension",
        AuthzMode::Disabled => "disabled",
        AuthzMode::Unrestricted => "unrestricted",
    }
}

fn build_sink(pool: Option<Arc<sqlx::PgPool>>) -> Arc<dyn AuthzAuditSink> {
    pool.map_or_else(
        || -> Arc<dyn AuthzAuditSink> { Arc::new(NullAuditSink) },
        |p| -> Arc<dyn AuthzAuditSink> {
            Arc::new(DbAuditSink::new(GovernanceDecisionRepository::from_pool(p)))
        },
    )
}
