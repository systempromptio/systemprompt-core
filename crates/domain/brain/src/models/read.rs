//! Read-side rows: recall receipts with their rendered block, outcome
//! signals, proposals, and the filter fact listings take.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_extension::brain::RecallConversation;
use systemprompt_identifiers::{FactKindId, UserId, WorkflowId};
use systemprompt_models::brain::{
    BrainOutcome, FactStatus, ProposalDraft, RecallAudit, RecallRerank, SubjectKey,
};
use systemprompt_models::origin::ClientKind;

pub type NewOutcome = BrainOutcome;
pub type NewProposal = ProposalDraft;

/// One recall to record: who asked, in which conversation, what was
/// injected or why it was skipped, the exact block that was sent and the
/// reranker that ordered it, if one ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRecall {
    pub principal: UserId,
    pub client: ClientKind,
    pub project: Option<String>,
    pub workflow: Option<WorkflowId>,
    pub conversation: RecallConversation,
    pub audit: RecallAudit,
    pub skipped: Option<String>,
    pub rendered_block: Option<String>,
    pub reranked: Option<RecallRerank>,
}

impl NewRecall {
    #[must_use]
    pub const fn builder(
        principal: UserId,
        client: ClientKind,
        audit: RecallAudit,
    ) -> NewRecallBuilder {
        NewRecallBuilder {
            recall: Self {
                principal,
                client,
                project: None,
                workflow: None,
                conversation: RecallConversation {
                    ai_request_id: None,
                    gateway_conversation_id: None,
                    session_id: None,
                    context_id: None,
                },
                audit,
                skipped: None,
                rendered_block: None,
                reranked: None,
            },
        }
    }
}

/// Builds a [`NewRecall`] from its principal, client and audit.
#[derive(Debug, Clone)]
pub struct NewRecallBuilder {
    recall: NewRecall,
}

impl NewRecallBuilder {
    #[must_use]
    pub fn conversation(mut self, conversation: RecallConversation) -> Self {
        self.recall.conversation = conversation;
        self
    }

    #[must_use]
    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.recall.project = Some(project.into());
        self
    }

    #[must_use]
    pub fn workflow(mut self, workflow: WorkflowId) -> Self {
        self.recall.workflow = Some(workflow);
        self
    }

    #[must_use]
    pub fn skipped(mut self, reason: impl Into<String>) -> Self {
        self.recall.skipped = Some(reason.into());
        self
    }

    #[must_use]
    pub fn rendered_block(mut self, block: impl Into<String>) -> Self {
        self.recall.rendered_block = Some(block.into());
        self
    }

    #[must_use]
    pub fn reranked(mut self, reranked: RecallRerank) -> Self {
        self.recall.reranked = Some(reranked);
        self
    }

    #[must_use]
    pub fn build(self) -> NewRecall {
        self.recall
    }
}

/// Which facts a listing returns.
///
/// Empty `statuses` means every status. `as_of` keeps only facts observed at or
/// before that instant and valid then, as recall with the same `as_of` would
/// see them. `after` resumes a listing past a
/// [`FactCursor`](super::FactCursor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactFilter {
    pub kind: Option<FactKindId>,
    pub subject: Option<SubjectKey>,
    pub statuses: Vec<FactStatus>,
    pub limit: i64,
    pub as_of: Option<DateTime<Utc>>,
    pub after: Option<String>,
}

/// One page of a visible listing and the cursor of the next page, if the
/// listing may hold more.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FactPage {
    pub facts: Vec<systemprompt_models::brain::BrainFact>,
    pub next: Option<String>,
}

impl FactFilter {
    pub const DEFAULT_LIMIT: i64 = 100;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            kind: None,
            subject: None,
            statuses: Vec::new(),
            limit: Self::DEFAULT_LIMIT,
            as_of: None,
            after: None,
        }
    }

    #[must_use]
    pub fn with_kind(mut self, kind: FactKindId) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn with_subject(mut self, subject: SubjectKey) -> Self {
        self.subject = Some(subject);
        self
    }

    #[must_use]
    pub fn with_statuses(mut self, statuses: Vec<FactStatus>) -> Self {
        self.statuses = statuses;
        self
    }

    #[must_use]
    pub const fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    #[must_use]
    pub fn with_after(mut self, after: String) -> Self {
        self.after = Some(after);
        self
    }

    #[must_use]
    pub const fn with_as_of(mut self, as_of: DateTime<Utc>) -> Self {
        self.as_of = Some(as_of);
        self
    }
}

impl Default for FactFilter {
    fn default() -> Self {
        Self::new()
    }
}
