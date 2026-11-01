//! The brain sink of a server built or configured without the brain: it
//! reports itself unavailable, so the document store lands revisions and
//! resolves comments without recording a brain fact.
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
