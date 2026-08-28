//! Bounded ring of the requests the proxy governed, with emit hooks for the
//! GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

const RING_CAPACITY: usize = 500;

pub type EmitHook = Box<dyn Fn(&RequestRecord) + Send + Sync>;

/// What the bridge itself decided about a request, before the gateway saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalVerdict {
    Forwarded,
    Denied,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RequestRecord {
    pub id: u64,
    pub ts_unix: u64,
    pub req_id: String,
    pub agent: String,
    pub method: String,
    pub path: String,
    pub verdict: LocalVerdict,
    pub deny_reason: Option<String>,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub model: Option<String>,
    pub upstream_request_id: Option<String>,
    pub gateway_decision: Option<String>,
    pub gateway_policy: Option<String>,
}

#[derive(Debug)]
pub struct NewRequest<'a> {
    pub req_id: &'a str,
    pub agent: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub verdict: LocalVerdict,
    pub deny_reason: Option<String>,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub upstream_request_id: Option<String>,
}

/// The bridge's own record of what it forwarded and what it refused.
#[expect(
    missing_debug_implementations,
    reason = "holds Vec<Box<dyn Fn(&RequestRecord) + Send + Sync>> hooks; cannot derive Debug"
)]
#[derive(Clone)]
pub struct RequestLog {
    inner: Arc<Mutex<RingState>>,
    hooks: Arc<Mutex<Vec<EmitHook>>>,
}

struct RingState {
    next_id: u64,
    entries: VecDeque<RequestRecord>,
}

impl Default for RequestLog {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RingState {
                next_id: 1,
                entries: VecDeque::with_capacity(RING_CAPACITY),
            })),
            hooks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn record(&self, new: NewRequest<'_>) -> u64 {
        let record = {
            let mut g = self.inner.lock();
            let id = g.next_id;
            g.next_id += 1;
            let record = RequestRecord {
                id,
                ts_unix: now_unix(),
                req_id: new.req_id.to_owned(),
                agent: new.agent.to_owned(),
                method: new.method.to_owned(),
                path: new.path.to_owned(),
                verdict: new.verdict,
                deny_reason: new.deny_reason,
                status: new.status,
                latency_ms: new.latency_ms,
                tokens_in: None,
                tokens_out: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: None,
                upstream_request_id: new.upstream_request_id,
                gateway_decision: None,
                gateway_policy: None,
            };
            if g.entries.len() == RING_CAPACITY {
                g.entries.pop_front();
            }
            g.entries.push_back(record.clone());
            record
        };
        self.emit(&record);
        record.id
    }

    pub fn settle_usage(&self, req_id: &str, usage: SettledUsage) {
        let mut guard = self.inner.lock();
        let updated = {
            let g = &mut *guard;
            let Some(record) = g.entries.iter_mut().rev().find(|r| r.req_id == req_id) else {
                return;
            };
            record.tokens_in = Some(usage.input);
            record.tokens_out = Some(usage.output);
            record.cache_read_tokens = usage.cache_read;
            record.cache_write_tokens = usage.cache_write;
            record.model = usage.model;
            record.clone()
        };
        drop(guard);
        self.emit(&updated);
    }

    // Why: the gateway's verdict arrives out of band, keyed by the request id it
    // returned in `x-systemprompt-request-id` -- not by our own `req_id`.
    pub fn apply_gateway_decision(&self, upstream_request_id: &str, decision: &str, policy: &str) {
        let mut guard = self.inner.lock();
        let updated = {
            let g = &mut *guard;
            let Some(record) = g
                .entries
                .iter_mut()
                .rev()
                .find(|r| r.upstream_request_id.as_deref() == Some(upstream_request_id))
            else {
                return;
            };
            record.gateway_decision = Some(decision.to_owned());
            record.gateway_policy = Some(policy.to_owned());
            record.clone()
        };
        drop(guard);
        self.emit(&updated);
    }

    pub fn snapshot_recent(&self, limit: usize) -> Vec<RequestRecord> {
        let g = self.inner.lock();
        let start = g.entries.len().saturating_sub(limit);
        g.entries.iter().skip(start).cloned().collect()
    }

    pub fn add_emit_hook(&self, hook: EmitHook) {
        self.hooks.lock().push(hook);
    }

    fn emit(&self, record: &RequestRecord) {
        let hooks = self.hooks.lock();
        for hook in hooks.iter() {
            hook(record);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SettledUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub model: Option<String>,
}

static REQUEST_LOG: OnceLock<RequestLog> = OnceLock::new();

pub fn request_log() -> &'static RequestLog {
    REQUEST_LOG.get_or_init(RequestLog::new)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
