//! Pairing an MCP execution with the model's tool-call intent it fulfils.
//!
//! The intent lives in the ai domain and is claimed through the injected
//! [`ToolCallIntentClaims`](systemprompt_traits::ToolCallIntentClaims); the
//! claimed call id is then stamped onto this domain's execution row. The two
//! writes are separate transactions, so the service reconciles the one way
//! they can disagree: an intent still unclaimed on the ai side whose call id
//! an execution already carries (an exact execution recorded before its
//! intent row existed, or one whose own claim failed). Such an intent is
//! handed to the execution that holds it and the claim moves on to the next
//! candidate. Every execution row must exist before a claim names it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::{AiToolCallId, McpExecutionId, McpToolName, SessionId};
use systemprompt_traits::DynToolCallIntentClaims;

use crate::error::McpDomainResult;
use crate::repository::{IntentStamp, ToolUsageRepository};

const MAX_STALE_INTENTS: usize = 3;

#[derive(Clone)]
pub struct IntentClaimService {
    intents: DynToolCallIntentClaims,
    executions: Arc<ToolUsageRepository>,
}

impl fmt::Debug for IntentClaimService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntentClaimService")
            .field("executions", &self.executions)
            .finish_non_exhaustive()
    }
}

impl IntentClaimService {
    pub fn new(intents: DynToolCallIntentClaims, executions: Arc<ToolUsageRepository>) -> Self {
        Self {
            intents,
            executions,
        }
    }

    pub const fn executions(&self) -> &Arc<ToolUsageRepository> {
        &self.executions
    }

    pub async fn claim_exact(
        &self,
        call: &AiToolCallId,
        execution: &McpExecutionId,
    ) -> McpDomainResult<bool> {
        Ok(self.intents.claim(call, execution).await?)
    }

    pub async fn claim_inferred(
        &self,
        session_id: &SessionId,
        tool_name: &McpToolName,
        execution: &McpExecutionId,
        window_seconds: i64,
    ) -> McpDomainResult<Option<AiToolCallId>> {
        for _ in 0..MAX_STALE_INTENTS {
            let Some(call) = self
                .intents
                .claim_newest_unclaimed(session_id, tool_name, execution, window_seconds)
                .await?
            else {
                return Ok(None);
            };
            match self
                .executions
                .stamp_inferred_intent(execution, &call)
                .await
            {
                Ok(IntentStamp::Stamped) => return Ok(Some(call)),
                Ok(IntentStamp::HeldBy(holder)) => {
                    self.hand_over(&call, execution, &holder).await?;
                },
                Ok(IntentStamp::Refused) => {
                    self.intents.release(&call, execution).await?;
                    return Ok(None);
                },
                Err(stamp_error) => {
                    if let Err(release_error) = self.intents.release(&call, execution).await {
                        tracing::warn!(
                            %execution,
                            %call,
                            error = %release_error,
                            "Claimed intent could not be released after a failed stamp"
                        );
                    }
                    return Err(stamp_error);
                },
            }
        }
        tracing::warn!(
            %execution,
            %session_id,
            tool = %tool_name,
            attempts = MAX_STALE_INTENTS,
            "Every claimed intent was already held by another execution"
        );
        Ok(None)
    }

    async fn hand_over(
        &self,
        call: &AiToolCallId,
        from: &McpExecutionId,
        holder: &McpExecutionId,
    ) -> McpDomainResult<()> {
        self.intents.release(call, from).await?;
        let handed = self.intents.claim(call, holder).await?;
        tracing::info!(
            %call,
            claimant = %from,
            %holder,
            handed,
            "Intent already carried by another execution was reconciled to it"
        );
        Ok(())
    }
}
