//! The permission gate one tool call asks before anything with side effects:
//! the configured rules decide allow, deny or ask; an ask goes to the user; the
//! first allowed request then goes to the Governor for its verdict, and to the
//! admitted extensions' reviewers once the Governor allowed it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{Action, AgentDef, Loaded, Rule, Ruleset};
use crate::store::SessionId;
use crate::tools::{PermissionGate, PermissionRequest, ToolError};
use async_trait::async_trait;
use parking_lot::Mutex;
use systemprompt_identifiers::engine::PermissionId;
use systemprompt_traits::CancellationToken;
use tokio::sync::OnceCell;

use crate::run::events::{PermissionAsk, Reply};
use crate::run::governor::{ToolCall, Verdict};
use crate::run::inner::Inner;

pub(crate) struct Gate {
    inner: Arc<Inner>,
    session: SessionId,
    agent: AgentDef,
    config: Arc<Loaded>,
    governed: ToolCall,
    cancel: CancellationToken,
    verdict: OnceCell<Verdict>,
    policy: Mutex<Option<String>>,
    rejection_feedback: Mutex<Option<String>>,
    rule_denied: AtomicBool,
}

impl Gate {
    pub(crate) fn new(
        inner: Arc<Inner>,
        agent: AgentDef,
        config: Arc<Loaded>,
        governed: ToolCall,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            inner,
            session: governed.session_id.clone(),
            agent,
            config,
            governed,
            cancel,
            verdict: OnceCell::new(),
            policy: Mutex::new(None),
            rejection_feedback: Mutex::new(None),
            rule_denied: AtomicBool::new(false),
        }
    }

    pub(crate) fn policy_denial(&self) -> Option<String> {
        self.policy.lock().clone()
    }

    pub(crate) fn rejection_feedback(&self) -> Option<String> {
        self.rejection_feedback.lock().clone()
    }

    pub(crate) fn rule_denial(&self) -> bool {
        self.rule_denied.load(Ordering::Acquire)
    }

    pub(crate) fn reviewed(&self) -> bool {
        self.verdict.initialized()
    }

    async fn ask_user(&self, request: &PermissionRequest) -> Result<(), ToolError> {
        let ask = PermissionAsk {
            id: PermissionId::mint(self.inner.store.ids()),
            session: self.session.clone(),
            call_id: self.governed.call_id.clone(),
            tool: self.governed.tool.clone(),
            permission: request.permission.clone(),
            patterns: request.patterns.clone(),
            always: request.always.clone(),
            title: request.title.clone(),
            metadata: request.metadata.clone(),
        };
        match self
            .inner
            .asks
            .permission(&self.inner, ask, &self.cancel)
            .await
        {
            None => Err(ToolError::Aborted),
            Some(Reply::Once) => Ok(()),
            Some(Reply::Always) => {
                self.remember(request).await;
                Ok(())
            },
            Some(Reply::Reject { feedback }) => {
                *self.rejection_feedback.lock() = feedback;
                Err(ToolError::Rejected)
            },
        }
    }

    async fn remember(&self, request: &PermissionRequest) {
        let granted: Ruleset = request
            .always
            .iter()
            .map(|pattern| Rule::new(request.permission.clone(), pattern.clone(), Action::Allow))
            .collect();
        if let Err(error) = self.inner.add_session_rules(&self.session, &granted).await {
            tracing::warn!(session = %self.session, error = %error, "approval not remembered");
        }
    }

    async fn review(&self) -> Result<(), ToolError> {
        let verdict = self
            .verdict
            .get_or_init(|| async { self.inner.governed.tool_before(&self.governed).await })
            .await;
        match verdict {
            Verdict::Allow => Ok(()),
            Verdict::Deny { reason } => {
                *self.policy.lock() = Some(reason.clone());
                let tool = &self.governed.tool;
                tracing::info!(session = %self.session, tool = %tool, "tool call denied by policy");
                Err(ToolError::Denied {
                    reason: reason.clone(),
                })
            },
        }
    }
}

#[async_trait]
impl PermissionGate for Gate {
    async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError> {
        let session_rules = self
            .inner
            .session_rules(&self.session)
            .await
            .map_err(|error| ToolError::caused("could not read the session's approvals", error))?;
        let rules = self.config.rules_for(&self.agent, &session_rules);
        match rules.evaluate_all(
            &request.permission,
            request.patterns.iter().map(String::as_str),
        ) {
            Action::Deny => {
                self.rule_denied.store(true, Ordering::Release);
                let patterns = request.patterns.join(", ");
                let reason = format!("a rule denies {} for {patterns}", request.permission);
                return Err(ToolError::Denied { reason });
            },
            Action::Ask => self.ask_user(&request).await?,
            Action::Allow => {},
        }
        self.review().await
    }
}
