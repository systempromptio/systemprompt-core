//! The marketplace part of the fixtures: the managed repository the
//! `managed_resources` member provides into a context's kernel state, and a
//! test member that contributes a per-user catalogue filter to the
//! marketplace slot, as an install does.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_marketplace::contract::MarketplaceSlot;
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_runtime::AppContext;

pub fn fixture_managed_repository(ctx: &AppContext) -> &Arc<ManagedRepository> {
    ctx.kernel()
        .state()
        .get::<Arc<ManagedRepository>>()
        .expect("the managed_resources member provides the managed repository")
}

pub struct FilterMember {
    filter: Arc<dyn MarketplaceFilter>,
}

impl FilterMember {
    pub fn new(filter: Arc<dyn MarketplaceFilter>) -> Self {
        Self { filter }
    }
}

impl Extension for FilterMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "fixture-marketplace-filter",
            name: "Fixture marketplace filter",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        let filter = Arc::clone(&self.filter);
        c.slot::<MarketplaceSlot>()
            .filter(
                move |_: &Resolved<'_>| -> Result<Arc<dyn MarketplaceFilter>, FactoryError> {
                    Ok(Arc::clone(&filter))
                },
            );
        Ok(())
    }
}

pub fn fixture_admitted_with_filter(
    filter: Arc<dyn MarketplaceFilter>,
) -> Arc<systemprompt_extension::Admitted> {
    let mut members = crate::fixture_core_members();
    members.push(Box::new(FilterMember::new(filter)));
    crate::fixture_admitted_of(members)
}
