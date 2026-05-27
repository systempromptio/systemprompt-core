//! Construction of the active authz decision hook from profile config.
//!
//! [`build_authz_hook`] is the single entry point for both the API server and
//! any standalone MCP binary. It inspects `governance.authz` and returns an
//! `Arc<dyn AuthzDecisionHook>` that the caller stores on its `AppContext`
//! (or equivalent) and threads to every consumer.
//!
//! Branch table:
//!
//! - `mode: webhook` with a non-empty `url` that passes SSRF validation →
//!   [`WebhookHook`] (fail-closed) ahead of [`RuleBasedHook`] under a
//!   [`CompositeAuthzHook`].
//! - `mode: extension` with one or more extension hooks (`AppContextBuilder::with_authz_hook`
//!   or `register_authz_hook!`) → composite `[RuleBasedHook, ...extensions]`.
//!   Bootstrap fails with [`AuthzBootstrapError::ExtensionModeButNoHook`] if
//!   no extension hook is supplied.
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
//! `governance_decisions` and [`RuleBasedHook`] can query
//! `access_control_rules`.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_models::net::validate_outbound_url;
use systemprompt_models::profile::{AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT};

use super::audit::{AuthzAuditSink, DbAuditSink, GovernanceDecisionRepository, NullAuditSink};
use super::composite::CompositeAuthzHook;
use super::error::{AuthzBootstrapError, AuthzResult};
use super::hook::{AllowAllHook, DenyAllHook, SharedAuthzHook, WebhookHook};
use super::registry::{AuthzHookContext, discover_authz_hook};
use super::rule_based::RuleBasedHook;

pub fn build_authz_hook(
    governance: Option<&GovernanceConfig>,
    pool: Option<Arc<sqlx::PgPool>>,
    extension: Option<SharedAuthzHook>,
) -> AuthzResult<SharedAuthzHook> {
    let sink = build_sink(pool.clone());

    let extension = extension.or_else(|| {
        pool.as_ref().and_then(|p| {
            discover_authz_hook(&AuthzHookContext {
                pool: Arc::clone(p),
                sink: Arc::clone(&sink),
            })
        })
    });

    let Some(authz) = governance.and_then(|g| g.authz.as_ref()) else {
        if extension.is_some() {
            return Err(AuthzBootstrapError::NoGovernanceButExtensionHook.into());
        }
        tracing::error!(
            "governance.authz block missing — using DenyAllHook (all requests will be denied)"
        );
        return Ok(Arc::new(DenyAllHook::new(sink)));
    };

    match (authz.hook.mode, extension) {
        (AuthzMode::Extension, Some(hook)) => {
            tracing::info!(
                "governance.authz.hook.mode = extension — composing RuleBasedHook + {:?}",
                hook
            );
            Ok(compose_rule_based(pool, sink, vec![hook]))
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
            let webhook = WebhookHook::new(
                url,
                Duration::from_millis(authz.hook.timeout_ms),
                Arc::clone(&sink),
            )?;
            let webhook: SharedAuthzHook = Arc::new(webhook);
            Ok(compose_rule_based(pool, sink, vec![webhook]))
        },
    }
}

fn compose_rule_based(
    pool: Option<Arc<sqlx::PgPool>>,
    sink: Arc<dyn AuthzAuditSink>,
    mut tail: Vec<SharedAuthzHook>,
) -> SharedAuthzHook {
    // Why: RuleBasedHook needs the DbPool to query access_control_rules. When
    // no pool is available (pre-DB bootstrap, tests), skip it — the resolver
    // cannot run without rule storage, so composing it would only deny.
    let Some(pool) = pool else {
        if tail.len() == 1 {
            return tail.remove(0);
        }
        return Arc::new(CompositeAuthzHook::new(tail));
    };
    let rule_based: SharedAuthzHook = Arc::new(RuleBasedHook::new(pool, sink));
    let mut hooks = Vec::with_capacity(tail.len() + 1);
    hooks.push(rule_based);
    hooks.extend(tail);
    Arc::new(CompositeAuthzHook::new(hooks))
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
