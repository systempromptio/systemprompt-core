//! Compile-time registration of [`MarketplaceFilter`] implementations.
//!
//! Extensions submit a [`MarketplaceFilterRegistration`] via the
//! [`register_marketplace_filter!`] macro. [`discover_filters`] returns every
//! submission ordered by descending `priority`; ties resolve by submission
//! order (deterministic per build). With no registration the runtime falls back
//! to [`crate::AllowAllFilter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;

use crate::error::MarketplaceFilterError;
use crate::filter::MarketplaceFilter;

type MarketplaceFilterFactory =
    fn(&DbPool) -> Result<Arc<dyn MarketplaceFilter>, MarketplaceFilterError>;

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
