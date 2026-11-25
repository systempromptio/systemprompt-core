//! The profile usage contract: one user's rolling token usage and
//! conversation summary, read by a surface outside the analytics capability.
//!
//! The `analytics` member owns the derivation and provides it **sealed** as
//! [`DynProfileUsageSource`]: a read names any user. The `gateway-http`
//! member reads it in its provide fn for the bridge profile usage route; a
//! composition without the `analytics` member provides none, and that route
//! then answers 503 naming the absent member.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_models::api::cloud::BridgeProfileUsage;
use systemprompt_traits::BoxedSource;

#[derive(Debug, thiserror::Error)]
#[error("the profile usage read failed: {0}")]
pub struct ProfileUsageError(#[source] pub BoxedSource);

/// One user's profile usage windows.
///
/// `#[async_trait]` because the source is held as `Arc<dyn
/// ProfileUsageSource>`.
#[async_trait]
pub trait ProfileUsageSource: Send + Sync {
    async fn profile_usage(
        &self,
        user: &UserId,
        now: DateTime<Utc>,
    ) -> Result<BridgeProfileUsage, ProfileUsageError>;
}

pub type DynProfileUsageSource = Arc<dyn ProfileUsageSource>;
