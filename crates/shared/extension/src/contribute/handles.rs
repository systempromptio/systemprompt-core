//! The kernel handles a conferral can hand a route set: the [`KernelHandle`]
//! contract, the [`Handed`] route-state value that reads one, and the closed
//! [`HandleId`] set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_traits::events::AnalyticsSubscription;

use systemprompt_provider_contracts::MissingDependency;

use super::resolved::{FromState, Resolved};

/// The analytics event broadcaster, subscribe-only.
#[derive(Debug, Clone, Copy)]
pub struct AnalyticsBroadcaster;

impl KernelHandle for AnalyticsBroadcaster {
    type Value = Arc<dyn AnalyticsSubscription>;
    const ID: HandleId = HandleId::AnalyticsBroadcaster;
}

/// A kernel-built value handed to exactly the routes it is conferred on.
pub trait KernelHandle: 'static {
    type Value: Clone + Send + Sync + 'static;
    const ID: HandleId;
}

pub struct Handed<K: KernelHandle>(pub K::Value);

impl<K: KernelHandle> Clone for Handed<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K: KernelHandle> FromState for Handed<K> {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        r.handed::<K>().map(Self)
    }
}

impl<K: KernelHandle> std::fmt::Debug for Handed<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Handed").field(&K::ID).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleId {
    AnalyticsBroadcaster,
}
