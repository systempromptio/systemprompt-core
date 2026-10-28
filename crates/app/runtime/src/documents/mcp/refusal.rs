//! Refusals: tool-level failures an agent can act on.
//!
//! A refusal is a tool result with `isError: true`, never a JSON-RPC error.
//! Its `structuredContent` carries a stable [`RefusalCode`] and whatever
//! typed detail the failure knows (the head revision and who wrote it, a
//! node's current text, the kinds a format accepts); the retry prose stays
//! in `content[0].text`. Handlers raise a refusal as an [`McpError`] whose
//! `data` is the serialised [`Refusal`]; [`refusal_result`] turns it back
//! into the tool result at the dispatch boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode};
use serde::Serialize;
use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId, UserId};
use systemprompt_models::documents::ActorKind;

/// The stable code of a refusal; `documents_guide` explains each as
/// `refusal.<code>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    StaleBase,
    GuardMismatch,
    FindNone,
    UnsupportedOp,
    UnsupportedFormat,
    TemplateFailed,
    Forbidden,
    NotFound,
    Locked,
    AnchorsAffected,
    ProposalConflict,
    InvalidParams,
}

impl RefusalCode {
    pub const ALL: [Self; 12] = [
        Self::StaleBase,
        Self::GuardMismatch,
        Self::FindNone,
        Self::UnsupportedOp,
        Self::UnsupportedFormat,
        Self::TemplateFailed,
        Self::Forbidden,
        Self::NotFound,
        Self::Locked,
        Self::AnchorsAffected,
        Self::ProposalConflict,
        Self::InvalidParams,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleBase => "stale_base",
            Self::GuardMismatch => "guard_mismatch",
            Self::FindNone => "find_none",
            Self::UnsupportedOp => "unsupported_op",
            Self::UnsupportedFormat => "unsupported_format",
            Self::TemplateFailed => "template_failed",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Locked => "locked",
            Self::AnchorsAffected => "anchors_affected",
            Self::ProposalConflict => "proposal_conflict",
            Self::InvalidParams => "invalid_params",
        }
    }
}

/// The `structuredContent` of a refused tool call.
#[derive(Debug, Clone, Serialize)]
pub struct Refusal {
    pub code: RefusalCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written_by: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_kind: Option<ActorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<DocumentNodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<DocumentCommentId>>,
}

impl Refusal {
    #[must_use]
    pub const fn new(code: RefusalCode) -> Self {
        Self {
            code,
            head_revision: None,
            written_by: None,
            actor_kind: None,
            at: None,
            node: None,
            hash: None,
            current_text: None,
            matches: None,
            accepted_kinds: None,
            owner: None,
            trashed: None,
            comments: None,
        }
    }

    #[must_use]
    pub fn into_error(self, message: impl Into<String>) -> McpError {
        let data = match serde_json::to_value(&self) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(error = %error, code = self.code.as_str(), "refusal detail was not serialised");
                None
            },
        };
        McpError::invalid_params(message.into(), data)
    }
}

#[must_use]
pub fn refusal(code: RefusalCode, message: impl Into<String>) -> McpError {
    Refusal::new(code).into_error(message)
}

#[must_use]
pub fn refusal_code(error: &McpError) -> Option<RefusalCode> {
    let tagged = error
        .data
        .as_ref()
        .and_then(|data| data.get("code"))
        .and_then(serde_json::Value::as_str);
    tagged
        .and_then(|code| RefusalCode::ALL.into_iter().find(|c| c.as_str() == code))
        .or_else(|| (error.code == ErrorCode::INVALID_PARAMS).then_some(RefusalCode::InvalidParams))
}

#[must_use]
pub fn refusal_result(error: &McpError) -> Option<CallToolResult> {
    let tagged = error
        .data
        .as_ref()
        .filter(|data| data.get("code").is_some_and(serde_json::Value::is_string));
    let structured = match tagged {
        Some(data) => data.clone(),
        None if error.code == ErrorCode::INVALID_PARAMS => {
            serde_json::to_value(Refusal::new(RefusalCode::InvalidParams)).ok()?
        },
        None => return None,
    };
    let mut result = CallToolResult::error(vec![ContentBlock::text(error.message.to_string())]);
    result.structured_content = Some(structured);
    Some(result)
}
