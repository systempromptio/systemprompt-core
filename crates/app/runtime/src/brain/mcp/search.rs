//! The read tools: search the facts the caller may see, explain one fact's
//! in-context score, and preview what the gateway would inject for a
//! prompt. The preview runs the recall service for a new turn and never
//! records the recall.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::Utc;
use rmcp::ErrorData as McpError;
use systemprompt_brain::{BrainError, FactFilter};
use systemprompt_extension::brain::{
    RecallConversation, RecallOutcome, RecallPrincipal, RecallRequest, ReuseHint,
};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::brain::{BrainFact, RecallCtx, SubjectKey};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::origin::ClientKind;

use super::refusal::brain_tool_error;
use super::tools::{
    FactItem, RecallPreviewInput, RecallPreviewOutput, SearchInput, SearchOutput, WhyInput,
    WhyOutput,
};
use super::{BrainMcpTools, agent_actor, brain_caller};

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 50;
const SCAN_LIMIT: i64 = 200;

fn matches(fact: &BrainFact, input: &SearchInput, needle: &str) -> bool {
    let in_scope = input.scope.is_none_or(|level| fact.scope.level() == level);
    let text = needle.is_empty()
        || fact.statement.to_lowercase().contains(needle)
        || fact.subject.to_string().to_lowercase().contains(needle);
    in_scope && text
}

#[derive(Debug, Clone, Copy)]
pub struct SearchHandler<'a> {
    pub tools: &'a BrainMcpTools,
}

impl McpToolHandler for SearchHandler<'_> {
    type Input = SearchInput;
    type Output = SearchOutput;

    fn tool_name(&self) -> &'static str {
        "brain_search"
    }

    fn description(&self) -> &'static str {
        "Search team memory you may see. Guide: facts."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: SearchInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(SearchOutput, String), McpError> {
        let caller = brain_caller(ctx)?;
        let mut filter = FactFilter::new().with_limit(SCAN_LIMIT);
        if let Some(kind) = &input.kind {
            filter = filter.with_kind(kind.clone());
        }
        if let Some(subject) = &input.subject {
            let key: SubjectKey = subject
                .parse()
                .map_err(|error| brain_tool_error(BrainError::Subject(error)))?;
            filter = filter.with_subject(key);
        }
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let needle = input.query.trim().to_lowercase();
        let facts: Vec<FactItem> = self
            .tools
            .service()
            .search(&caller, &filter)
            .await
            .map_err(brain_tool_error)?
            .into_iter()
            .filter(|fact| matches(fact, &input, &needle))
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .map(FactItem::from)
            .collect();
        let summary = format!("{} facts", facts.len());
        Ok((SearchOutput { facts }, summary))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WhyHandler<'a> {
    pub tools: &'a BrainMcpTools,
}

impl McpToolHandler for WhyHandler<'_> {
    type Input = WhyInput;
    type Output = WhyOutput;

    fn tool_name(&self) -> &'static str {
        "brain_why"
    }

    fn description(&self) -> &'static str {
        "Why a fact scores as it does, and whether recall withholds it."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: WhyInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(WhyOutput, String), McpError> {
        let caller = brain_caller(ctx)?;
        let mut recall_ctx = RecallCtx::new(caller.user_id.clone(), agent_actor(ctx), Utc::now());
        if let Some(project) = input.project {
            recall_ctx = recall_ctx.with_project(project);
        }
        let why = self
            .tools
            .service()
            .why(&caller, &input.fact_id, &recall_ctx)
            .await
            .map_err(brain_tool_error)?;
        let output = WhyOutput {
            fact_id: why.fact.id,
            revision: why.fact.revision,
            base: why.terms.base,
            subject: why.terms.subject,
            workflow: why.terms.workflow,
            recency: why.terms.recency,
            total: why.terms.total,
            withheld: why.withheld.map(|reason| reason.as_str().to_owned()),
        };
        let summary = format!("score {:.3}", output.total);
        Ok((output, summary))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RecallPreviewHandler<'a> {
    pub tools: &'a BrainMcpTools,
}

impl RecallPreviewHandler<'_> {
    fn request(
        &self,
        input: &RecallPreviewInput,
        ctx: &RequestContext,
    ) -> Result<RecallRequest, McpError> {
        let caller = brain_caller(ctx)?;
        let recall = &self.tools.profile().recall;
        let principal = RecallPrincipal {
            user_id: caller.user_id,
            client: ClientKind::Other,
            actor: agent_actor(ctx),
            project: input.project.clone(),
            client_id: ctx.client_id().cloned(),
        };
        Ok(RecallRequest::new(principal, RecallConversation::default())
            .with_subject_hints(&input.prompt)
            .with_budget_tokens(recall.budget_tokens)
            .with_deadline(Duration::from_millis(recall.deadline_ms))
            .with_reuse(ReuseHint::NewTurn))
    }
}

impl McpToolHandler for RecallPreviewHandler<'_> {
    type Input = RecallPreviewInput;
    type Output = RecallPreviewOutput;

    fn tool_name(&self) -> &'static str {
        "brain_recall_preview"
    }

    fn description(&self) -> &'static str {
        "What the gateway would inject for a prompt. Read-only."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: RecallPreviewInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(RecallPreviewOutput, String), McpError> {
        let request = self.request(&input, ctx)?;
        let outcome = self.tools.recall().recall(&request).await.map_err(|error| {
            tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "brain recall preview failed");
            McpError::internal_error("the brain recall failed; try the call again", None)
        })?;
        let summary = outcome.descriptor();
        let output = match outcome {
            RecallOutcome::Injected(block) => RecallPreviewOutput {
                injected: true,
                block: Some(block.render()),
                skipped: None,
                withheld: block
                    .withheld
                    .by_reason
                    .iter()
                    .map(|(reason, count)| (reason.as_str().to_owned(), *count))
                    .collect(),
                tokens: block.tokens,
            },
            RecallOutcome::Skipped { reason } => RecallPreviewOutput {
                injected: false,
                block: None,
                skipped: Some(reason.as_str().to_owned()),
                withheld: std::collections::BTreeMap::new(),
                tokens: 0,
            },
        };
        Ok((output, summary))
    }
}
