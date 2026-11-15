//! [`MarketplaceSlot`]: the marketplace's contribution point.
//!
//! A member contributes a per-user catalogue filter with `.filter(..)` and a
//! publication gate with `.publication_gate(..)`; each method stores a factory
//! and runs nothing. Admission drops a disabled member's builder before any
//! factory runs. The slot is [`SlotOrder::CoreFirst`], and both lists are
//! all-must-pass, so the outcome does not depend on order: every filter
//! narrows the candidate in turn and any filter error refuses it, and every
//! gate must admit a gated publication. With no filter contributed the built
//! filter is [`AllowAllFilter`]. A factory that fails, or reads a value
//! nothing provides, refuses the build: a filter or gate is never silently
//! left out.
//!
//! The built [`MarketplaceSeams`] also carries the governance role directory
//! the publication policy resolves cohort roles through, read from the
//! built `RoleDirectories`. The `managed_resources` member opens the slot, so
//! the built seams are present exactly when that member is admitted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    Factory, FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder,
};
use systemprompt_governance::authz::SharedRoleDirectory;
use systemprompt_governance::contract::RoleDirectories;
use systemprompt_identifiers::{ExtensionId, UserId};

use crate::candidate::MarketplaceCandidate;
use crate::error::MarketplaceFilterError;
use crate::filter::{AllowAllFilter, MarketplaceFilter};
use crate::managed::PublicationGate;

const SLOT: &str = "marketplace";

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// The marketplace's contribution point, owned by the marketplace
/// capability.
#[derive(Debug, Clone, Copy)]
pub struct MarketplaceSlot;

/// One member's filters and publication gates, stored until the slot is
/// built.
#[derive(Default)]
pub struct MarketplaceSlotBuilder {
    filters: Factories<dyn MarketplaceFilter>,
    gates: Factories<dyn PublicationGate>,
}

impl MarketplaceSlotBuilder {
    pub fn filter<F: Factory<Arc<dyn MarketplaceFilter>>>(&mut self, f: F) -> &mut Self {
        self.filters.push(Box::new(f));
        self
    }

    pub fn publication_gate<F: Factory<Arc<dyn PublicationGate>>>(&mut self, f: F) -> &mut Self {
        self.gates.push(Box::new(f));
        self
    }
}

impl fmt::Debug for MarketplaceSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarketplaceSlotBuilder")
            .field("filters", &self.filters.len())
            .field("gates", &self.gates.len())
            .finish()
    }
}

/// The built marketplace seams: one filter applying every contributed
/// filter, the publication gates core first, and the role directory.
#[derive(Clone)]
pub struct MarketplaceSeams {
    filter: Arc<dyn MarketplaceFilter>,
    gates: Vec<Arc<dyn PublicationGate>>,
    roles: Option<SharedRoleDirectory>,
}

impl MarketplaceSeams {
    #[must_use]
    pub fn filter(&self) -> Arc<dyn MarketplaceFilter> {
        Arc::clone(&self.filter)
    }

    #[must_use]
    pub fn gates(&self) -> &[Arc<dyn PublicationGate>] {
        &self.gates
    }

    #[must_use]
    pub fn roles(&self) -> Option<SharedRoleDirectory> {
        self.roles.clone()
    }
}

impl fmt::Debug for MarketplaceSeams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarketplaceSeams")
            .field("filter", &self.filter)
            .field(
                "gates",
                &self.gates.iter().map(|g| g.id()).collect::<Vec<_>>(),
            )
            .field("roles", &self.roles.is_some())
            .finish()
    }
}

impl Slot for MarketplaceSlot {
    type Builder = MarketplaceSlotBuilder;
    type Built = MarketplaceSeams;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut filters = Vec::new();
        let mut gates = Vec::new();
        for entry in entries {
            for factory in &entry.builder.filters {
                filters.push(run(factory.as_ref(), &entry.owner, r)?);
            }
            for factory in &entry.builder.gates {
                gates.push(run(factory.as_ref(), &entry.owner, r)?);
            }
        }
        let filter: Arc<dyn MarketplaceFilter> = match filters.len() {
            0 => Arc::new(AllowAllFilter),
            1 => filters.remove(0),
            _ => Arc::new(AllFilters { filters }),
        };
        let roles = r.get::<Arc<RoleDirectories>>()?.directory();
        Ok(MarketplaceSeams {
            filter,
            gates,
            roles,
        })
    }
}

/// Every contributed filter applied in turn; the first error refuses.
#[derive(Debug)]
struct AllFilters {
    filters: Vec<Arc<dyn MarketplaceFilter>>,
}

#[async_trait]
impl MarketplaceFilter for AllFilters {
    async fn filter(
        &self,
        user_id: &UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        let mut candidate = candidate;
        for filter in &self.filters {
            candidate = filter.filter(user_id, candidate).await?;
        }
        Ok(candidate)
    }
}

fn run<T: ?Sized + 'static>(
    factory: &dyn Factory<Arc<T>>,
    owner: &ExtensionId,
    r: &Resolved<'_>,
) -> Result<Arc<T>, SlotError> {
    factory.build(r).map_err(|error| match error {
        FactoryError::Missing(missing) => SlotError::Missing(missing),
        FactoryError::Failed(source) => SlotError::Entry {
            slot: SLOT,
            owner: owner.clone(),
            source,
        },
    })
}
