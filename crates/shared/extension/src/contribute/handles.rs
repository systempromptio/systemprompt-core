//! The kernel handles a conferral can hand a route set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_traits::events::AnalyticsSubscription;

use super::resolved::{HandleId, KernelHandle};

/// The analytics event broadcaster, subscribe-only.
#[derive(Debug, Clone, Copy)]
pub struct AnalyticsBroadcaster;

impl KernelHandle for AnalyticsBroadcaster {
    type Value = Arc<dyn AnalyticsSubscription>;
    const ID: HandleId = HandleId::AnalyticsBroadcaster;
}
