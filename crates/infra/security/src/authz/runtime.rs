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

/// Zero-sized witness that [`install_global_hook`] has run at least once.
/// Downstream router builders take `&AuthzHookInstalled` as a parameter,
/// making "build router before installing hook" a compile error rather than
/// a silent fail-closed at request time.
#[derive(Debug, Clone, Copy)]
pub struct AuthzHookInstalled {
    _private: (),
}

pub fn install_global_hook(hook: SharedHook) -> AuthzHookInstalled {
    if let Ok(mut guard) = slot().write() {
        *guard = Some(hook);
    }
    AuthzHookInstalled { _private: () }
}

pub fn clear_global_hook() {
    if let Ok(mut guard) = slot().write() {
        *guard = None;
    }
}

/// Internal accessor. Always returns `Some` after [`install_global_hook`] has
/// run; the [`AuthzHookInstalled`] witness is the public proof that bootstrap
/// completed, so call sites should consume the witness and unwrap
/// unconditionally.
#[must_use]
pub fn global_hook() -> Option<SharedHook> {
    slot().read().ok().and_then(|g| g.clone())
}

pub fn install_from_governance_config(
    governance: Option<&GovernanceConfig>,
    pool: Option<Arc<sqlx::PgPool>>,
) -> AuthzResult<AuthzHookInstalled> {
    let sink = build_sink(pool);

    let Some(authz) = governance.and_then(|g| g.authz.as_ref()) else {
        tracing::error!(
            "governance.authz block missing — installing DenyAllHook (all requests will be denied)"
        );
        return Ok(install_global_hook(Arc::new(DenyAllHook::new(sink))));
    };

    match authz.hook.mode {
        AuthzMode::Disabled => {
            tracing::warn!("governance.authz.hook.mode = disabled — all requests will be denied");
            Ok(install_global_hook(Arc::new(DenyAllHook::new(sink))))
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
            Ok(install_global_hook(Arc::new(AllowAllHook::new(sink))))
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
            Ok(install_global_hook(Arc::new(hook)))
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
