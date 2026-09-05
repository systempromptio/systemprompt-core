//! Terminal-signal correction for the Anthropic passthrough lane.
//!
//! Same-wire Anthropic in to Anthropic out relays the upstream body unparsed,
//! so the canonical `with_tool_use` correction never runs. An upstream that
//! declares `end_turn` beside a `tool_use` block therefore reaches the client
//! as a finished turn and the call is silently dropped. This module keeps the
//! lane byte-faithful for a consistent body and rewrites exactly the
//! `stop_reason` token when the body contradicts itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
// JSON: protocol boundary — the shapes inspected here are the Anthropic wire's.
use serde_json::Value;

const TOOL_USE: &str = "tool_use";

// Why: mirrors `CanonicalStopReason::with_tool_use` -- truncation and an
// explicit stop sequence still win, because a call cut mid-arguments is not a
// call the client can run.
fn is_generic_stop(reason: &str) -> bool {
    !matches!(reason, TOOL_USE | "max_tokens" | "stop_sequence")
}

// Why: the client-visible payload must not change except for the one token
// that is wrong, so the rewrite is textual -- re-serialising the parsed value
// would renormalise whitespace and number formatting across the whole body.
fn rewrite_stop_reason(raw: &[u8], old: &str) -> Option<Vec<u8>> {
    let key = b"\"stop_reason\"";
    let mut from = 0_usize;
    while let Some(found) = find(raw, key, from) {
        let mut i = found + key.len();
        while raw.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if raw.get(i) != Some(&b':') {
            from = found + 1;
            continue;
        }
        i += 1;
        while raw.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let quoted = format!("\"{old}\"");
        if raw.get(i..i + quoted.len()) == Some(quoted.as_bytes()) {
            let mut out = Vec::with_capacity(raw.len());
            out.extend_from_slice(&raw[..i]);
            out.extend_from_slice(b"\"tool_use\"");
            out.extend_from_slice(&raw[i + quoted.len()..]);
            return Some(out);
        }
        from = found + 1;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn has_tool_use_block(value: &Value) -> bool {
    value["content"]
        .as_array()
        .is_some_and(|blocks| blocks.iter().any(|b| b["type"] == TOOL_USE))
}

// Why: the buffered passthrough body, corrected only when it contradicts
// itself; a consistent body is returned as the very bytes that arrived.
pub(in crate::services::gateway) fn correct_buffered(body: Bytes) -> Bytes {
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return body;
    };
    let Some(reason) = value["stop_reason"].as_str() else {
        return body;
    };
    if !has_tool_use_block(&value) || !is_generic_stop(reason) {
        return body;
    }
    rewrite_stop_reason(&body, reason).map_or(body, Bytes::from)
}

#[derive(Debug, Default)]
struct StreamState {
    buf: Vec<u8>,
    saw_tool_use: bool,
}

impl StreamState {
    // Why: SSE frames arrive split across chunks, so correction has to see
    // whole frames; every byte is re-emitted, only re-grouped by frame.
    fn push(&mut self, chunk: &[u8]) -> Vec<Bytes> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(end) = systemprompt_models::wire::sse::frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            out.push(self.correct_frame(frame));
        }
        out
    }

    fn flush(&mut self) -> Option<Bytes> {
        (!self.buf.is_empty()).then(|| Bytes::from(std::mem::take(&mut self.buf)))
    }

    fn correct_frame(&mut self, frame: Vec<u8>) -> Bytes {
        let Some(value) = frame_json(&frame) else {
            return Bytes::from(frame);
        };
        if value["type"] == "content_block_start" && value["content_block"]["type"] == TOOL_USE {
            self.saw_tool_use = true;
        }
        if value["type"] != "message_delta" || !self.saw_tool_use {
            return Bytes::from(frame);
        }
        let Some(reason) = value["delta"]["stop_reason"].as_str() else {
            return Bytes::from(frame);
        };
        if !is_generic_stop(reason) {
            return Bytes::from(frame);
        }
        rewrite_stop_reason(&frame, reason).map_or_else(|| Bytes::from(frame), Bytes::from)
    }
}

fn frame_json(frame: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(frame);
    text.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find(|data| data.trim() != "[DONE]")
        .and_then(|data| serde_json::from_str::<Value>(data).ok())
}

// Why: the streaming twin of `correct_buffered` -- the tool-use block is
// announced before the `message_delta` that closes the turn, so the state
// needed to spot the contradiction is always in hand by then.
pub(in crate::services::gateway) fn correct_stream<S>(
    stream: S,
) -> BoxStream<'static, Result<Bytes, String>>
where
    S: futures_util::Stream<Item = Result<Bytes, String>> + Send + 'static,
{
    use futures_util::stream;
    stream
        .map(Some)
        .chain(stream::once(async { None }))
        .scan(StreamState::default(), |state, item| {
            let res = match item {
                Some(Ok(bytes)) => state.push(&bytes).into_iter().map(Ok).collect(),
                Some(Err(e)) => vec![Err(e)],
                None => state.flush().map(Ok).into_iter().collect(),
            };
            futures_util::future::ready(Some(res))
        })
        .flat_map(stream::iter)
        .boxed()
}
