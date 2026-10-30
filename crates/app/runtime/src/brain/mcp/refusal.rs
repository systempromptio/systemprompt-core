//! Refusals over MCP: a tool result with `isError: true` whose
//! `structuredContent` is `{"code": ...}` and whose text says what to do.
//!
//! A handler raises a refusal as an [`McpError`] carrying the code in its
//! `data`; [`refusal_result`] turns it back into the tool result at the
//! dispatch boundary. Every [`BrainError`] maps to one code through
//! [`refusal_of`]; a transient failure has none and is logged and answered
//! with a fixed message as a protocol error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode};
use serde_json::json;
use systemprompt_brain::BrainError;

/// Why a brain tool refused a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrainRefusalCode {
    BrainDisabled,
    UnknownKind,
    Forbidden,
    AgentForbidden,
    NotFound,
    Tombstoned,
    InvalidInput,
}

impl BrainRefusalCode {
    pub const ALL: [Self; 7] = [
        Self::BrainDisabled,
        Self::UnknownKind,
        Self::Forbidden,
        Self::AgentForbidden,
        Self::NotFound,
        Self::Tombstoned,
        Self::InvalidInput,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrainDisabled => "brain_disabled",
            Self::UnknownKind => "unknown_kind",
            Self::Forbidden => "forbidden",
            Self::AgentForbidden => "agent_forbidden",
            Self::NotFound => "not_found",
            Self::Tombstoned => "tombstoned",
            Self::InvalidInput => "invalid_input",
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|known| known.as_str() == code)
    }
}

#[must_use]
pub fn refusal(code: BrainRefusalCode, message: impl Into<String>) -> McpError {
    McpError::invalid_params(message.into(), Some(json!({ "code": code.as_str() })))
}

#[must_use]
pub const fn refusal_of(error: &BrainError) -> Option<BrainRefusalCode> {
    match error {
        BrainError::UnknownKind(_) => Some(BrainRefusalCode::UnknownKind),
        BrainError::Tombstoned => Some(BrainRefusalCode::Tombstoned),
        BrainError::Forbidden { .. } => Some(BrainRefusalCode::Forbidden),
        BrainError::NotFound(_) => Some(BrainRefusalCode::NotFound),
        BrainError::Statement(_) | BrainError::Subject(_) => Some(BrainRefusalCode::InvalidInput),
        BrainError::Validate(_) => Some(BrainRefusalCode::InvalidInput),
        BrainError::Database(_)
        | BrainError::Repository(_)
        | BrainError::Encode(_)
        | BrainError::Sink(_)
        | BrainError::Notify(_)
        | BrainError::NotifyKind(_)
        | BrainError::UnknownEvent(_)
        | BrainError::Extract(_)
        | BrainError::Source(_)
        | BrainError::SourceTimeout
        | BrainError::BudgetExhausted => None,
    }
}

#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "handlers pass it as the fn pointer of map_err over an owned BrainError"
)]
pub fn brain_tool_error(error: BrainError) -> McpError {
    if let Some(code) = refusal_of(&error) {
        return refusal(code, error.to_string());
    }
    tracing::error!(
        error = %error,
        cause = ?std::error::Error::source(&error),
        "brain tool failed"
    );
    McpError::internal_error("the brain store failed; try the call again", None)
}

#[must_use]
pub fn refusal_code(error: &McpError) -> Option<BrainRefusalCode> {
    let tagged = error
        .data
        .as_ref()
        .and_then(|data| data.get("code"))
        .and_then(serde_json::Value::as_str);
    tagged.and_then(BrainRefusalCode::parse).or_else(|| {
        (error.code == ErrorCode::INVALID_PARAMS).then_some(BrainRefusalCode::InvalidInput)
    })
}

#[must_use]
pub fn refusal_result(error: &McpError) -> Option<CallToolResult> {
    let code = refusal_code(error)?;
    let mut result = CallToolResult::error(vec![ContentBlock::text(error.message.to_string())]);
    result.structured_content = Some(json!({ "code": code.as_str() }));
    Some(result)
}
