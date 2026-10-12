//! The terminal outcome a gateway request is settled with: token usage, the
//! completion's payload, assistant text and tool calls, or what a failure
//! still consumed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::ai_request_payloads::UpsertPayloadParams;
use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone, Copy, Default)]
pub struct SettlementUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub reasoning_tokens: u32,
    pub tokens_used: u32,
}

#[derive(Debug, Clone)]
pub struct SettledToolCall {
    pub id: AiToolCallId,
    pub name: String,
    pub input: String,
}

#[derive(Debug)]
pub struct SettleCompletion<'a> {
    pub usage: SettlementUsage,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<&'a str>,
    pub payload: UpsertPayloadParams<'a>,
    pub assistant_text: Option<&'a str>,
    pub tool_calls: &'a [SettledToolCall],
    /// The sequence number the assistant message takes: the count of
    /// messages admission wrote, carried from memory. `None` only for a
    /// receipt journaled before the count was carried; its assistant text is
    /// then not recorded rather than numbered by re-reading the ledger.
    pub next_message_sequence: Option<i32>,
}

/// What a failed request still consumed.
///
/// A provider bills the tokens it streamed before the stream broke, so a
/// failure that carries usage is settled with it: recording only the error
/// leaves that spend invisible to every cost view, quota bucket and rollup.
/// `usage` is `None` when nothing was observed, and then the stored columns
/// are left untouched.
#[derive(Debug, Default)]
pub struct SettledFailure<'a> {
    pub error: &'a str,
    pub usage: Option<SettlementUsage>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
}

#[derive(Debug)]
pub enum SettlementOutcome<'a> {
    Completed(SettleCompletion<'a>),
    Failed(SettledFailure<'a>),
}
