//! Refusals over MCP: a tool-level failure an agent can act on.
//!
//! A refusal is a tool result with `isError: true`, never a JSON-RPC error.
//! Its `structuredContent` is the [`Refusal`]; the retry prose stays in
//! `content[0].text`. Handlers raise a refusal as an [`McpError`] whose
//! `data` is the serialised [`Refusal`]; [`refusal_result`] turns it back
//! into the tool result at the dispatch boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode};

use crate::documents::refusal::{Refusal, RefusalCode, Refused};

#[must_use]
pub fn refused_error(refused: Refused) -> McpError {
    let Refused { refusal, message } = refused;
    let data = match serde_json::to_value(&refusal) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(error = %error, code = refusal.code.as_str(), "refusal detail was not serialised");
            None
        },
    };
    McpError::invalid_params(message, data)
}

#[must_use]
pub fn refusal(code: RefusalCode, message: impl Into<String>) -> McpError {
    refused_error(Refusal::new(code).with_message(message))
}

#[must_use]
pub fn refusal_code(error: &McpError) -> Option<RefusalCode> {
    let tagged = error
        .data
        .as_ref()
        .and_then(|data| data.get("code"))
        .and_then(serde_json::Value::as_str);
    tagged
        .and_then(RefusalCode::parse)
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
