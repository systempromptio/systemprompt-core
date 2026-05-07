use std::sync::Arc;

use systemprompt_database::DbPool;

use crate::error::MarketplaceFilterError;
use crate::filter::MarketplaceFilter;

/// Factory function passed to [`register_marketplace_filter!`].
///
/// Receives the live database pool so implementations can hold a
/// repository, connection cache, or any other DB-backed handle they
/// need to evaluate ACL at request time. Construction may fail (for
/// example if the pool is not Postgres) — failures are surfaced as
/// [`MarketplaceFilterError`] and the runtime logs them and falls back
/// to [`crate::AllowAllFilter`].
pub type MarketplaceFilterFactory =
    fn(&DbPool) -> Result<Arc<dyn MarketplaceFilter>, MarketplaceFilterError>;

/// Inventory submission slot for [`MarketplaceFilter`] implementations.
///
/// Extensions register a filter at compile time by calling
/// [`register_marketplace_filter!`]. The runtime builder collects every
/// submission and selects the one with the highest [`priority`] field —
/// ties resolve by submission order, which is deterministic per build.
/// If no extension registers a filter, the runtime falls back to
/// [`crate::AllowAllFilter`].
///
/// [`priority`]: MarketplaceFilterRegistration::priority
#[derive(Debug, Clone, Copy)]
pub struct MarketplaceFilterRegistration {
    pub factory: MarketplaceFilterFactory,
    pub priority: i32,
}

inventory::collect!(MarketplaceFilterRegistration);

#[must_use]
pub fn discover_filters() -> Vec<&'static MarketplaceFilterRegistration> {
    let mut all: Vec<&MarketplaceFilterRegistration> =
        inventory::iter::<MarketplaceFilterRegistration>().collect();
    all.sort_by_key(|reg| std::cmp::Reverse(reg.priority));
    all
}

/// Register a [`crate::MarketplaceFilter`] implementation with the runtime.
///
/// Pairs with the `inventory` collection slot in this crate. Higher
/// `priority` values win when multiple filters are registered.
///
/// ```ignore
/// use systemprompt_marketplace::register_marketplace_filter;
/// register_marketplace_filter!(MyFilter::new, priority = 100);
/// ```
#[macro_export]
macro_rules! register_marketplace_filter {
    ($factory:expr, priority = $priority:expr $(,)?) => {
        ::inventory::submit! {
            $crate::MarketplaceFilterRegistration {
                factory: $factory,
                priority: $priority,
            }
        }
    };
    ($factory:expr $(,)?) => {
        $crate::register_marketplace_filter!($factory, priority = 0);
    };
}
