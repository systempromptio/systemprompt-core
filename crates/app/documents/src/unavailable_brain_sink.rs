//! The brain sink the document store writes through: the `brain` member's
//! outbox sink while the profile enables the brain, otherwise
//! [`BrainUnavailableSink`], which reports itself unavailable, so the
//! document store lands revisions and resolves comments without recording a
//! brain fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{BrainSink, BrainSinkError};
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::BrainEvent;

/// A [`BrainSink`] that records nothing and says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainUnavailableSink;

#[async_trait::async_trait]
impl BrainSink for BrainUnavailableSink {
    fn is_available(&self) -> bool {
        false
    }

    async fn emit(
        &self,
        _conn: &mut sqlx::PgConnection,
        _event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError> {
        Err(BrainSinkError::Unavailable)
    }
}
