//! The checked pushes a member's `Contributions` makes into its own sink:
//! a slot builder found or opened by the slot's `TypeId`, and a route set
//! or gateway route whose kind error is recorded beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

#[cfg(feature = "router")]
use super::error::ContributionError;
use super::sink::ContributionSink;
use super::slot::{Slot, SlotRecord};

impl ContributionSink {
    pub(super) fn slot_builder<S: Slot>(&mut self) -> &mut S::Builder {
        let slots = &mut self.slots;
        let index = slots
            .iter()
            .position(|record| record.slot() == TypeId::of::<S>())
            .unwrap_or_else(|| {
                slots.push(SlotRecord::new::<S>());
                slots.len() - 1
            });
        slots[index]
            .builder_mut::<S>()
            .unwrap_or_else(|| unreachable!("slot records are keyed by the slot's TypeId"))
    }

    #[cfg(feature = "router")]
    pub(super) fn push_route_set(&mut self, mut r: crate::router::RouteSet) {
        if let Some(error) = r.take_error() {
            self.errors.push(error);
        }
        if let base @ crate::router::BaseRef::Gateway(_) = r.base() {
            self.errors.push(ContributionError::RouteSetKind(base));
        }
        self.routes.push(r);
    }

    #[cfg(feature = "router")]
    pub(super) fn push_gateway_route(&mut self, mut r: crate::router::GatewayRouteDecl) {
        if let Some(error) = r.router.take_error() {
            self.errors.push(error);
        }
        if let base @ crate::router::BaseRef::Path(_) = r.router.base() {
            self.errors.push(ContributionError::RouteSetKind(base));
        }
        self.gateway_routes.push(r);
    }
}
