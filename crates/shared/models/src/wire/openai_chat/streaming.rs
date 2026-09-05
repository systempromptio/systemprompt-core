//! `OpenAI` Chat Completions SSE-to-[`CanonicalEvent`] translation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use core::fmt::Display;

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::Value;
use systemprompt_identifiers::MessageId;

use super::stream_delta::{
    OpenAiChatStreamState, close_reasoning, process_reasoning_delta, process_text_delta,
    process_tool_calls,
};
use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate,
};

// Why: the codec has to act on the end of the upstream stream, not only on
// its frames -- a turn whose `finish_reason` was seen but whose usage chunk
// never arrived still has to state its terminal. `scan` cannot observe the
// end, so the end is made a frame.
enum Frame {
    Chunk(Result<Bytes, String>),
    Eof,
}

pub fn sse_to_canonical_events<S, E>(
    stream: S,
    fallback_model: String,
) -> BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Display,
{
    let initial = OpenAiChatStreamState {
        buf: Vec::new(),
        model: fallback_model,
        message_id: MessageId::new(""),
        started: false,
        text_block: None,
        next_index: 0,
        tool_calls: Vec::new(),
        reasoning_block: None,
        saw_tool_call: false,
        stopped: false,
        pending_finish: None,
    };

    let s = stream
        .map(|chunk| match chunk {
            Ok(bytes) => Frame::Chunk(Ok(bytes)),
            Err(e) => Frame::Chunk(Err(e.to_string())),
        })
        .chain(stream::once(futures_util::future::ready(Frame::Eof)))
        .scan(initial, |state, item| {
            let res = match item {
                Frame::Chunk(Ok(bytes)) => drain_buffer(state, &bytes),
                Frame::Chunk(Err(e)) => vec![Err(e)],
                Frame::Eof => flush(state),
            };
            futures_util::future::ready(Some(res))
        })
        .flat_map(stream::iter);
    s.boxed()
}

fn drain_buffer(
    state: &mut OpenAiChatStreamState,
    bytes: &Bytes,
) -> Vec<Result<CanonicalEvent, String>> {
    state.buf.extend_from_slice(bytes);
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    while let Some(end) = crate::wire::sse::frame_end(&state.buf) {
        let frame: Vec<u8> = state.buf.drain(..end).collect();
        let frame_str = String::from_utf8_lossy(&frame);
        for line in frame_str.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                flush_into(state, &mut events);
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            handle_chunk(state, &value, &mut events);
        }
    }
    events
}

// Why: a stream that ended without `[DONE]` still stated a finish reason on
// its last content chunk, and a turn that never stated one at all is a
// truncation the gateway reports separately -- so the flush states only what
// the wire actually said.
fn flush(state: &mut OpenAiChatStreamState) -> Vec<Result<CanonicalEvent, String>> {
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    flush_into(state, &mut events);
    events
}

fn flush_into(state: &mut OpenAiChatStreamState, events: &mut Vec<Result<CanonicalEvent, String>>) {
    if state.stopped {
        return;
    }
    let Some(finish) = state.pending_finish.take() else {
        return;
    };
    emit_message_stop(state, &finish, events);
}

fn handle_chunk(
    state: &mut OpenAiChatStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    // Why: chat completions reports a mid-stream failure as an `{"error":
    // ...}` chunk with no `choices`, which every branch below skips -- the
    // stream then reached `[DONE]` (or simply ended) with the failure dropped.
    if let Some(message) = crate::wire::sse::upstream_error_message(value) {
        events.push(Ok(CanonicalEvent::Error(message)));
        // Why: `[DONE]` still follows the error frame, and the sentinel
        // synthesises a `stop` terminal -- which would report the failed turn
        // as a clean finish to the client and to the audit.
        state.stopped = true;
        return;
    }
    if !state.started {
        emit_message_start(state, value, events);
    }
    if let Some(usage) = value.get("usage") {
        events.push(Ok(CanonicalEvent::UsageDelta(usage_from_value(usage))));
    }
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    else {
        return;
    };
    let delta = choice.get("delta").unwrap_or(&Value::Null);
    process_reasoning_delta(state, delta, events);
    process_text_delta(state, delta, events);
    process_tool_calls(state, delta, events);
    // Why: Chat Completions sends usage in a chunk of its own AFTER the one
    // carrying `finish_reason`, so a terminal emitted on sight of the finish
    // reason ends the canonical turn before its own counts arrive -- every
    // inbound surface then renders the turn with zeroed usage and the real
    // numbers, which the audit records, never reach the caller. The reason is
    // held until the stream states its end.
    if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str)
        && !state.stopped
        && state.pending_finish.is_none()
    {
        state.pending_finish = Some(finish.to_owned());
    }
}

fn emit_message_start(
    state: &mut OpenAiChatStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg_openai")
        .to_owned();
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&state.model)
        .to_owned();
    state.message_id = MessageId::new(&id);
    events.push(Ok(CanonicalEvent::MessageStart {
        id,
        model: model.clone(),
        usage: CanonicalUsage::default(),
    }));
    state.model = model;
    state.started = true;
}

fn emit_message_stop(
    state: &mut OpenAiChatStreamState,
    finish: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    state.stopped = true;
    close_reasoning(state, events);
    if let Some(index) = state.text_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
    for tc in state.tool_calls.drain(..) {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index: tc.index }));
    }
    events.push(Ok(CanonicalEvent::MessageStop {
        id: state.message_id.as_str().to_owned(),
        stop_reason: Some(
            CanonicalStopReason::from_openai(finish).with_tool_use(state.saw_tool_call),
        ),
    }));
}

fn usage_from_value(usage: &Value) -> CanonicalUsageUpdate {
    let field = |name: &str| usage.get(name).and_then(Value::as_u64).map(|v| v as u32);
    let cached = usage
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    // Why: `cached_tokens` is a subset of `prompt_tokens` here, but
    // `CanonicalUsage::input_tokens` is exclusive of cache reads, so the
    // streamed frame subtracts exactly as the buffered parse does.
    CanonicalUsageUpdate {
        input_tokens: field("prompt_tokens").map(|input| input.saturating_sub(cached.unwrap_or(0))),
        output_tokens: field("completion_tokens"),
        cache_read_tokens: cached,
        cache_creation_tokens: None,
        total_tokens: field("total_tokens"),
        // Why: already inside `completion_tokens` on this contract, so it is
        // reported as a breakdown and never added to the total.
        reasoning_tokens: usage
            .get("completion_tokens_details")
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(Value::as_u64)
            .map(|v| v as u32),
    }
}
