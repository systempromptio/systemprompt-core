//! Process-wide authz hook installed at server startup.
//!
//! Both the gateway `/v1/messages` middleware and the MCP RBAC middleware
//! consult [`global_hook`] to retrieve the active hook. After
//! [`install_from_governance_config`] runs the slot is always populated with
//! one of [`DenyAllHook`], [`AllowAllHook`], or [`WebhookHook`] — there is no
//! "uninstalled" path that callers can fall through.
//!
//! [`install_from_governance_config`] is the single source of truth for both
//! the API server runtime and standalone MCP server binaries:
//!
//! - `mode: webhook` with a non-empty `url` → [`WebhookHook`] (fail-closed).
//! - `mode: disabled`, or governance/authz absent → [`DenyAllHook`].
//! - `mode: unrestricted` → [`AllowAllHook`], but ONLY when `acknowledgement`
//!   exactly equals [`UNRESTRICTED_ACKNOWLEDGEMENT`]. Otherwise bootstrap
//!   fails.
//!
//! Bootstrap ordering: this is called from `AppContextBuilder::build` after
//! the database pool is created so the audit sink can write to
//! `governance_decisions`.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use systemprompt_models::profile::{AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT};

use super::audit::{AuthzAuditSink, DbAuditSink, GovernanceDecisionRepository, NullAuditSink};
use super::error::{AuthzBootstrapError, AuthzResult};
use super::hook::{AllowAllHook, AuthzDecisionHook, DenyAllHook, WebhookHook};

type SharedHook = Arc<dyn AuthzDecisionHook>;

fn slot() -> &'static RwLock<Option<SharedHook>> {
    static SLOT: OnceLock<RwLock<Option<SharedHook>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

pub fn install_global_hook(hook: SharedHook) {
    if let Ok(mut guard) = slot().write() {
        *guard = Some(hook);
    }
}

pub fn clear_global_hook() {
    if let Ok(mut guard) = slot().write() {
        *guard = None;
    }
}

#[must_use]
pub fn global_hook() -> Option<SharedHook> {
    slot().read().ok().and_then(|g| g.clone())
}

pub fn install_from_governance_config(
    governance: Option<&GovernanceConfig>,
    pool: Option<Arc<sqlx::PgPool>>,
) -> AuthzResult<()> {
    let sink = build_sink(pool);

    let Some(authz) = governance.and_then(|g| g.authz.as_ref()) else {
        tracing::error!(
            "governance.authz block missing — installing DenyAllHook (all requests will be denied)"
        );
        install_global_hook(Arc::new(DenyAllHook::new(sink)));
        return Ok(());
    };

    match authz.hook.mode {
        AuthzMode::Disabled => {
            tracing::warn!("governance.authz.hook.mode = disabled — all requests will be denied");
            install_global_hook(Arc::new(DenyAllHook::new(sink)));
            Ok(())
        },
        AuthzMode::Unrestricted => {
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
            install_global_hook(Arc::new(AllowAllHook::new(sink)));
            Ok(())
        },
        AuthzMode::Webhook => {
            let url = authz
                .hook
                .url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(AuthzBootstrapError::MissingWebhookUrl)?
                .to_owned();
            let hook = WebhookHook::new(url, Duration::from_millis(authz.hook.timeout_ms), sink)?;
            install_global_hook(Arc::new(hook));
            Ok(())
        },
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
