//! Inventory-based registration for extension-built authz hooks.
//!
//! Companion to [`AppContextBuilder::with_authz_hook`][with]: binaries that
//! delegate to `systemprompt::cli::run()` have no builder site to call, so
//! they register a hook factory at static-init time via
//! [`crate::register_authz_hook!`]. [`build_authz_hook`][bah] consults this
//! registry when no builder-supplied hook is present and the profile selects
//! `mode: extension`.
//!
//! Multiple registrations are auto-composed into a [`CompositeAuthzHook`] in
//! collection order. For deterministic ordering across many extensions,
//! register a single factory that returns a pre-composed hook.
//!
//! [with]: ../../../runtime/struct.AppContextBuilder.html#method.with_authz_hook
//! [bah]: super::runtime::build_authz_hook

use std::sync::Arc;

use super::audit::AuthzAuditSink;
use super::composite::CompositeAuthzHook;
use super::hook::SharedAuthzHook;

/// Inputs passed to every registered factory at bootstrap.
///
/// `pool` is the write-side Postgres pool already used by the audit sink;
/// `sink` is the same [`DbAuditSink`][super::audit::DbAuditSink] core uses
/// internally so extension hooks record through one consistent audit path.
#[derive(Clone)]
pub struct AuthzHookContext {
    pub pool: Arc<sqlx::PgPool>,
    pub sink: Arc<dyn AuthzAuditSink>,
}

impl std::fmt::Debug for AuthzHookContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthzHookContext").finish_non_exhaustive()
    }
}

/// One inventory submission per [`crate::register_authz_hook!`] call. The
/// factory runs once at `AppContext` build time and must not block.
#[derive(Debug, Clone, Copy)]
pub struct AuthzHookRegistration {
    pub factory: fn(&AuthzHookContext) -> SharedAuthzHook,
}

inventory::collect!(AuthzHookRegistration);

/// Returns the composed extension hook from every
/// [`crate::register_authz_hook!`] submission in the binary, or `None` if no
/// submissions exist.
#[must_use]
pub fn discover_authz_hook(ctx: &AuthzHookContext) -> Option<SharedAuthzHook> {
    let hooks: Vec<SharedAuthzHook> = inventory::iter::<AuthzHookRegistration>()
        .map(|reg| (reg.factory)(ctx))
        .collect();
    match hooks.len() {
        0 => None,
        1 => hooks.into_iter().next(),
        _ => Some(Arc::new(CompositeAuthzHook::new(hooks))),
    }
}

/// Register an extension authz hook factory at static-init time.
///
/// The factory receives a borrowed [`AuthzHookContext`] (pool + audit sink)
/// and returns the constructed hook. Wire alongside `register_extension!`
/// in the extension's `extension.rs`:
///
/// ```ignore
/// systemprompt_security::register_authz_hook!(|ctx| {
///     std::sync::Arc::new(MyHook::new(ctx.pool.clone(), ctx.sink.clone()))
///         as systemprompt_security::authz::SharedAuthzHook
/// });
/// ```
#[macro_export]
macro_rules! register_authz_hook {
    ($factory:expr) => {
        ::inventory::submit! {
            $crate::authz::AuthzHookRegistration {
                factory: $factory,
            }
        }
    };
}
