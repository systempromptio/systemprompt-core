//! The context directory of a host that admits no agent member: no user
//! owns any context, so the document store attributes no work to a
//! conversation and never names one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_traits::{
    CallerContexts, ContextDirectory, ContextProviderError, OwnedContextSummary,
};

/// A [`ContextDirectory`] in which no context is owned.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextsUnavailable;

impl ContextDirectory for ContextsUnavailable {
    fn for_caller(&self, _caller: &UserId) -> Box<dyn CallerContexts + '_> {
        Box::new(Self)
    }
}

#[async_trait::async_trait]
impl CallerContexts for ContextsUnavailable {
    async fn owned_context(
        &self,
        _context_id: &ContextId,
    ) -> Result<Option<OwnedContextSummary>, ContextProviderError> {
        Ok(None)
    }

    async fn owned_names(
        &self,
        _context_ids: &[ContextId],
    ) -> Result<HashMap<ContextId, String>, ContextProviderError> {
        Ok(HashMap::new())
    }
}
