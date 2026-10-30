//! The per-source readability check recall applies to every fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::notifications::SubjectRef;

use super::errors::SourceAccessError;

/// Whether a principal may see a fact's source: `AgentCapped` when the
/// person may read it but the acting agent may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceVerdict {
    Readable,
    Hidden,
    AgentCapped,
}

/// Decides whether a reader, acting as an actor, may read a source
/// resource; injected at the composition root as `Arc<dyn SourceAccess>`.
/// An error, a timeout or an unknown kind withholds the fact.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait SourceAccess: Send + Sync {
    async fn readable(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
        actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError>;
}
