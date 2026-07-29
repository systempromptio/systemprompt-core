//! Per-request connection scoping for pooled multi-tenancy.
//!
//! Extensions register a [`ConnectionScopeProvider`] that translates a
//! [`RequestScope`](systemprompt_models::RequestScope) into transaction-local
//! Postgres settings (custom GUCs such as `app.current_org`) applied via
//! `set_config($1, $2, true)` on a scoped transaction — the parameterizable
//! equivalent of `SET LOCAL`, which cannot bind parameters. Row-level-security
//! policies then read the setting with `current_setting('app.current_org',
//! true)`.
//!
//! The seam is strictly opt-in: only
//! [`begin_scoped`](crate::services::begin_scoped) and the
//! `with_scoped_transaction*` wrappers in
//! [`crate::services`] consult the registry, and with no registered provider
//! they degenerate to a plain `pool.begin()`. Existing transaction and pool
//! APIs are untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod provider;
mod registry;

pub use provider::{ConnectionScopeProvider, ScopeError, ScopeSetting, SharedScopeProvider};
pub use registry::{ScopeProviderRegistration, discover_scope_providers, scope_providers};
