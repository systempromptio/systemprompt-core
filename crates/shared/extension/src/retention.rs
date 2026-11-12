//! Retention holds: an extension's claim on operational rows it has not yet
//! released, honoured by the scheduler's `database_cleanup` job.
//!
//! Every [`RetentionTarget`] is pruned by age: rows whose timestamp column is
//! before the target's cutoff go. A hold answers, per target, a watermark `W`
//! meaning "every row before `W` is released; rows at or after `W` are not".
//! The job clamps the target's cutoff to the earliest watermark of every
//! registered hold, so a row at or after any hold's watermark survives the run
//! and is pruned on a later run once the hold advances. A hold that cannot
//! answer withholds the whole target for that run: the target is skipped,
//! never pruned past an unknown watermark.
//!
//! An exporter that has copied every `logs` row before its sync point:
//!
//! ```ignore
//! #[derive(Default)]
//! struct WarehouseHold;
//!
//! #[async_trait::async_trait]
//! impl RetentionHold for WarehouseHold {
//!     fn name(&self) -> &'static str {
//!         "warehouse-sync"
//!     }
//!
//!     async fn watermark(
//!         &self,
//!         target: RetentionTarget,
//!     ) -> Result<Option<DateTime<Utc>>, RetentionHoldError> {
//!         match target {
//!             RetentionTarget::Logs => load_sync_point().await.map(Some),
//!             _ => Ok(None),
//!         }
//!     }
//! }
//!
//! register_retention_hold!(WarehouseHold, priority = 100);
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::context::ExtensionContext;
use crate::seam::seam_registration;

/// The operational tables the retention job prunes, each by one timestamp
/// column compared with `<` against the target's cutoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetentionTarget {
    Logs,
    AnalyticsEvents,
    AiRequestMessages,
    McpToolExecutions,
    EventOutbox,
    AiRequestPayloads,
    GovernanceDecisions,
    AiQuotaBuckets,
    EngagementEvents,
    FingerprintReputation,
}

impl RetentionTarget {
    pub const ALL: [Self; 10] = [
        Self::Logs,
        Self::AnalyticsEvents,
        Self::AiRequestMessages,
        Self::McpToolExecutions,
        Self::EventOutbox,
        Self::AiRequestPayloads,
        Self::GovernanceDecisions,
        Self::AiQuotaBuckets,
        Self::EngagementEvents,
        Self::FingerprintReputation,
    ];

    #[must_use]
    pub const fn table(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::AnalyticsEvents => "analytics_events",
            Self::AiRequestMessages => "ai_request_messages",
            Self::McpToolExecutions => "mcp_tool_executions",
            Self::EventOutbox => "event_outbox",
            Self::AiRequestPayloads => "ai_request_payloads",
            Self::GovernanceDecisions => "governance_decisions",
            Self::AiQuotaBuckets => "ai_quota_buckets",
            Self::EngagementEvents => "engagement_events",
            Self::FingerprintReputation => "fingerprint_reputation",
        }
    }

    #[must_use]
    pub const fn timestamp_column(self) -> &'static str {
        match self {
            Self::Logs | Self::AnalyticsEvents => "timestamp",
            Self::AiQuotaBuckets => "window_start",
            Self::FingerprintReputation => "last_seen_at",
            Self::AiRequestMessages
            | Self::McpToolExecutions
            | Self::EventOutbox
            | Self::AiRequestPayloads
            | Self::GovernanceDecisions
            | Self::EngagementEvents => "created_at",
        }
    }
}

impl fmt::Display for RetentionTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.table())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RetentionHoldError {
    #[error("retention hold watermark unavailable")]
    Unavailable(#[source] Box<dyn StdError + Send + Sync>),
}

/// A claim on rows an extension has not yet released. `#[async_trait]`
/// keeps the trait `dyn`-compatible for the registry.
#[async_trait]
pub trait RetentionHold: Send + Sync {
    fn name(&self) -> &'static str;

    async fn watermark(
        &self,
        target: RetentionTarget,
    ) -> Result<Option<DateTime<Utc>>, RetentionHoldError>;
}

seam_registration!(
    RetentionHoldRegistration,
    RetentionHold,
    discover_retention_holds
);

#[macro_export]
macro_rules! register_retention_hold {
    ($($args:tt)*) => {
        $crate::__register_seam!(
            retention, RetentionHoldRegistration, RetentionHold, $($args)*
        );
    };
}

/// The cutoff a target is pruned to after every hold has answered.
#[derive(Debug)]
pub enum EffectiveCutoff {
    Age(DateTime<Utc>),
    Held {
        cutoff: DateTime<Utc>,
        hold: &'static str,
    },
    Withheld {
        hold: &'static str,
        error: RetentionHoldError,
    },
}

impl EffectiveCutoff {
    #[must_use]
    pub const fn cutoff(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Age(cutoff) | Self::Held { cutoff, .. } => Some(*cutoff),
            Self::Withheld { .. } => None,
        }
    }
}

/// The set of holds one retention run consults.
#[derive(Clone, Default)]
pub struct RetentionHolds {
    holds: Vec<Arc<dyn RetentionHold>>,
}

impl fmt::Debug for RetentionHolds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.holds.iter().map(|hold| hold.name()))
            .finish()
    }
}

impl RetentionHolds {
    #[must_use]
    pub const fn new(holds: Vec<Arc<dyn RetentionHold>>) -> Self {
        Self { holds }
    }

    #[must_use]
    pub fn discover(ctx: &dyn ExtensionContext) -> Self {
        Self::new(
            discover_retention_holds()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
        )
    }

    #[must_use]
    pub fn merged(mut self, other: &Self) -> Self {
        self.holds.extend(other.holds.iter().cloned());
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.holds.is_empty()
    }

    pub async fn resolve(
        &self,
        target: RetentionTarget,
        age_cutoff: DateTime<Utc>,
    ) -> EffectiveCutoff {
        let mut effective = EffectiveCutoff::Age(age_cutoff);
        for hold in &self.holds {
            match hold.watermark(target).await {
                Err(error) => {
                    return EffectiveCutoff::Withheld {
                        hold: hold.name(),
                        error,
                    };
                },
                Ok(Some(watermark)) => {
                    if effective
                        .cutoff()
                        .is_some_and(|current| watermark < current)
                    {
                        effective = EffectiveCutoff::Held {
                            cutoff: watermark,
                            hold: hold.name(),
                        };
                    }
                },
                Ok(None) => {},
            }
        }
        effective
    }
}
