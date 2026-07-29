//! Inventory-based registration for [`ConnectionScopeProvider`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

use super::provider::SharedScopeProvider;

/// One inventory submission per [`crate::register_scope_provider!`] call.
#[derive(Debug, Clone, Copy)]
pub struct ScopeProviderRegistration {
    pub factory: fn() -> SharedScopeProvider,
}

inventory::collect!(ScopeProviderRegistration);

/// Construct every registered provider, in collection order.
#[must_use]
pub fn discover_scope_providers() -> Vec<SharedScopeProvider> {
    inventory::iter::<ScopeProviderRegistration>()
        .map(|reg| (reg.factory)())
        .collect()
}

/// The process-wide provider list, constructed once on first use. An empty
/// slice (no extension registered) keeps every scoped-transaction call a
/// plain `pool.begin()`.
#[must_use]
pub fn scope_providers() -> &'static [SharedScopeProvider] {
    static PROVIDERS: OnceLock<Vec<SharedScopeProvider>> = OnceLock::new();
    PROVIDERS.get_or_init(discover_scope_providers)
}

/// Register a [`super::ConnectionScopeProvider`] factory at static-init time.
///
/// Wire alongside `register_extension!` in the extension's `extension.rs`:
///
/// ```ignore
/// systemprompt_database::register_scope_provider!(|| {
///     std::sync::Arc::new(OrgScopeProvider)
///         as systemprompt_database::scope::SharedScopeProvider
/// });
/// ```
#[macro_export]
macro_rules! register_scope_provider {
    ($factory:expr) => {
        ::inventory::submit! {
            $crate::scope::ScopeProviderRegistration {
                factory: $factory,
            }
        }
    };
}
